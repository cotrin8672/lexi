use crate::selection::{
    error_code, normalize_line_endings, CapturedSelection, SelectionCaptureError,
    SelectionCaptureFailure, SelectionDiagnostics,
};
use std::ffi::OsString;
use std::mem::size_of;
use std::os::windows::ffi::OsStringExt;
use std::sync::{mpsc, Mutex, OnceLock};
use std::thread;
use std::time::{Duration, Instant};
use windows::core::{Interface, BSTR, VARIANT};
use windows::Win32::Foundation::{CloseHandle, HGLOBAL, HWND, MAX_PATH};
use windows::Win32::System::Com::{CoCreateInstance, IDataObject, CLSCTX_INPROC_SERVER};
use windows::Win32::System::DataExchange::{
    CloseClipboard, EmptyClipboard, GetClipboardData, GetClipboardSequenceNumber,
    IsClipboardFormatAvailable, OpenClipboard,
};
use windows::Win32::System::Memory::{GlobalLock, GlobalSize, GlobalUnlock};
use windows::Win32::System::Ole::{
    OleGetClipboard, OleInitialize, OleSetClipboard, OleUninitialize,
};
use windows::Win32::System::ProcessStatus::K32GetModuleBaseNameW;
use windows::Win32::System::Threading::{
    OpenProcess, PROCESS_QUERY_LIMITED_INFORMATION, PROCESS_VM_READ,
};
use windows::Win32::UI::Accessibility::{
    CUIAutomation, IUIAutomation, IUIAutomationElement, IUIAutomationElementArray,
    IUIAutomationTextPattern, IUIAutomationTextRangeArray, TreeScope_Descendants,
    UIA_IsTextPatternAvailablePropertyId, UIA_TextPatternId,
};
use windows::Win32::UI::Input::KeyboardAndMouse::{
    GetAsyncKeyState, SendInput, INPUT, INPUT_0, INPUT_KEYBOARD, KEYBDINPUT, KEYEVENTF_KEYUP,
    VIRTUAL_KEY, VK_C, VK_CONTROL, VK_LWIN, VK_MENU, VK_RWIN, VK_SHIFT,
};
use windows::Win32::UI::WindowsAndMessaging::{
    GetForegroundWindow, GetWindowTextLengthW, GetWindowTextW, GetWindowThreadProcessId,
};

const CLIPBOARD_COPY_TIMEOUT: Duration = Duration::from_millis(250);
const CLIPBOARD_RESTORE_TIMEOUT: Duration = Duration::from_millis(300);
const CF_UNICODETEXT_FORMAT: u32 = 13;
const POLL_INTERVAL: Duration = Duration::from_millis(10);

struct ComApartment;

impl ComApartment {
    fn init() -> Result<Self, SelectionCaptureError> {
        unsafe {
            OleInitialize(None).map_err(map_windows_error)?;
        }
        Ok(Self)
    }
}

impl Drop for ComApartment {
    fn drop(&mut self) {
        unsafe {
            OleUninitialize();
        }
    }
}

pub fn capture_selected_text() -> Result<CapturedSelection, SelectionCaptureError> {
    capture_selected_text_with_context(None)
        .map(|result| result.selection)
        .map_err(|failure| failure.error)
}

pub fn capture_selected_text_with_failure() -> Result<CapturedSelection, SelectionCaptureFailure> {
    capture_selected_text_with_context(None).map(|result| result.selection)
}

pub fn capture_selected_text_with_clipboard_owner(
    clipboard_owner: isize,
) -> Result<CapturedSelection, SelectionCaptureFailure> {
    capture_selected_text_with_context(Some(clipboard_owner)).map(|result| result.selection)
}

pub fn capture_selection_diagnostics() -> SelectionDiagnostics {
    match capture_selected_text_with_context(None) {
        Ok(result) => {
            let character_count = result.selection.text.chars().count();
            SelectionDiagnostics {
                ok: true,
                code: "Ok".to_string(),
                capture_method: Some(result.selection.capture_method),
                source_process: result.selection.source_process,
                source_window_title: result.selection.source_window_title,
                character_count,
                multiline: result.selection.text.contains('\n'),
            }
        }
        Err(failure) => SelectionDiagnostics {
            ok: false,
            code: error_code(&failure.error).to_string(),
            capture_method: failure.capture_method,
            source_process: failure.source_process,
            source_window_title: failure.source_window_title,
            character_count: 0,
            multiline: false,
        },
    }
}

struct CaptureSuccess {
    selection: CapturedSelection,
}

type CaptureResult = Result<CaptureSuccess, SelectionCaptureFailure>;

struct CaptureSource {
    foreground_window: HWND,
    clipboard_owner: Option<HWND>,
    source_process: Option<String>,
    source_window_title: Option<String>,
}

struct BackendCapture {
    capture_method: &'static str,
    text: String,
}

trait CaptureBackend {
    fn name(&self) -> &'static str;

    fn capture(&mut self, source: &CaptureSource) -> Result<BackendCapture, BackendCaptureError>;
}

enum BackendCaptureError {
    Recoverable(SelectionCaptureError),
    Fatal(SelectionCaptureError),
}

impl From<SelectionCaptureError> for BackendCaptureError {
    fn from(error: SelectionCaptureError) -> Self {
        Self::Recoverable(error)
    }
}

struct CaptureRequest {
    clipboard_owner: Option<isize>,
    respond_to: mpsc::Sender<CaptureResult>,
}

struct SelectionWorker {
    requests: mpsc::Sender<CaptureRequest>,
}

static SELECTION_WORKER: OnceLock<Mutex<Option<SelectionWorker>>> = OnceLock::new();

struct CaptureContext<'a> {
    automation: &'a IUIAutomation,
    focused_element: IUIAutomationElement,
    source: &'a CaptureSource,
}

trait SelectionStrategy {
    fn name(&self) -> &'static str;

    fn capture(&self, context: &CaptureContext<'_>) -> Result<String, SelectionCaptureError>;
}

struct FocusedElementStrategy;

impl SelectionStrategy for FocusedElementStrategy {
    fn name(&self) -> &'static str {
        "uia-focused-element"
    }

    fn capture(&self, context: &CaptureContext<'_>) -> Result<String, SelectionCaptureError> {
        selected_text_from_element(context.automation, &context.focused_element)
    }
}

struct ForegroundWindowStrategy;

impl SelectionStrategy for ForegroundWindowStrategy {
    fn name(&self) -> &'static str {
        "uia-foreground-window"
    }

    fn capture(&self, context: &CaptureContext<'_>) -> Result<String, SelectionCaptureError> {
        let foreground_element = unsafe {
            context
                .automation
                .ElementFromHandle(context.source.foreground_window)
                .map_err(map_windows_error)?
        };
        selected_text_from_element(context.automation, &foreground_element)
    }
}

struct ClipboardBackend;

impl CaptureBackend for ClipboardBackend {
    fn name(&self) -> &'static str {
        "clipboard-copy"
    }

    fn capture(&mut self, source: &CaptureSource) -> Result<BackendCapture, BackendCaptureError> {
        let text = capture_text_by_clipboard_copy(source.clipboard_owner)?;
        Ok(BackendCapture {
            capture_method: self.name(),
            text,
        })
    }
}

struct UiaBackend {
    automation: IUIAutomation,
}

impl CaptureBackend for UiaBackend {
    fn name(&self) -> &'static str {
        "uia"
    }

    fn capture(&mut self, source: &CaptureSource) -> Result<BackendCapture, BackendCaptureError> {
        let focused = unsafe {
            self.automation.GetFocusedElement().map_err(|error| {
                match classify_windows_error(error.code()) {
                    SelectionCaptureError::AccessDenied => SelectionCaptureError::AccessDenied,
                    SelectionCaptureError::WindowsApiFailure(_) => {
                        SelectionCaptureError::FocusedElementUnavailable
                    }
                    other => other,
                }
            })?
        };
        let context = CaptureContext {
            automation: &self.automation,
            focused_element: focused,
            source,
        };
        let (capture_method, text) =
            run_uia_selection_strategies(&context).map_err(BackendCaptureError::Recoverable)?;

        Ok(BackendCapture {
            capture_method,
            text,
        })
    }
}

fn selection_strategies() -> [Box<dyn SelectionStrategy>; 2] {
    [
        Box::new(FocusedElementStrategy),
        Box::new(ForegroundWindowStrategy),
    ]
}

fn capture_selected_text_with_context(clipboard_owner: Option<isize>) -> CaptureResult {
    request_worker_capture(clipboard_owner)
}

fn request_worker_capture(clipboard_owner: Option<isize>) -> CaptureResult {
    let (respond_to, response) = mpsc::channel();
    let mut request = CaptureRequest {
        clipboard_owner,
        respond_to,
    };

    for attempt in 0..2 {
        let requests = selection_worker_sender();
        match requests.send(request) {
            Ok(()) => {
                return response.recv().unwrap_or_else(|_| {
                    Err(worker_failure(SelectionCaptureError::WindowsApiFailure(
                        "selection worker stopped before returning capture result".to_string(),
                    )))
                });
            }
            Err(error) if attempt == 0 => {
                reset_selection_worker();
                request = error.0;
            }
            Err(_) => {
                return Err(worker_failure(SelectionCaptureError::WindowsApiFailure(
                    "selection worker request channel is unavailable".to_string(),
                )));
            }
        }
    }

    Err(worker_failure(SelectionCaptureError::WindowsApiFailure(
        "selection worker retry exhausted".to_string(),
    )))
}

fn selection_worker_sender() -> mpsc::Sender<CaptureRequest> {
    let slot = SELECTION_WORKER.get_or_init(|| Mutex::new(None));
    let mut worker = slot.lock().expect("selection worker state poisoned");

    if worker.is_none() {
        *worker = Some(spawn_selection_worker());
    }

    worker
        .as_ref()
        .expect("selection worker should be initialized")
        .requests
        .clone()
}

fn reset_selection_worker() {
    if let Some(slot) = SELECTION_WORKER.get() {
        *slot.lock().expect("selection worker state poisoned") = None;
    }
}

fn spawn_selection_worker() -> SelectionWorker {
    let (requests, receiver) = mpsc::channel();
    thread::Builder::new()
        .name("lexi-selection-capture".to_string())
        .spawn(move || run_selection_worker(receiver))
        .expect("selection worker should start");

    SelectionWorker { requests }
}

fn run_selection_worker(receiver: mpsc::Receiver<CaptureRequest>) {
    let _com = match ComApartment::init() {
        Ok(com) => com,
        Err(error) => {
            respond_to_all_with_failure(receiver, error);
            return;
        }
    };

    let automation: IUIAutomation =
        match unsafe { CoCreateInstance(&CUIAutomation, None, CLSCTX_INPROC_SERVER) } {
            Ok(automation) => automation,
            Err(error) => {
                respond_to_all_with_failure(receiver, map_windows_error(error));
                return;
            }
        };
    let mut backends: Vec<Box<dyn CaptureBackend>> = vec![
        Box::new(ClipboardBackend),
        Box::new(UiaBackend { automation }),
    ];

    for request in receiver {
        let _ = request
            .respond_to
            .send(capture_selected_text_with_context_on_worker(
                &mut backends,
                request.clipboard_owner,
            ));
    }
}

fn respond_to_all_with_failure(
    receiver: mpsc::Receiver<CaptureRequest>,
    error: SelectionCaptureError,
) {
    for request in receiver {
        let _ = request.respond_to.send(Err(worker_failure(error.clone())));
    }
}

fn worker_failure(error: SelectionCaptureError) -> SelectionCaptureFailure {
    SelectionCaptureFailure {
        error,
        capture_method: None,
        source_process: None,
        source_window_title: None,
    }
}

fn capture_selected_text_with_context_on_worker(
    backends: &mut [Box<dyn CaptureBackend>],
    clipboard_owner: Option<isize>,
) -> CaptureResult {
    let source = capture_source(clipboard_owner)?;
    run_capture_backends(backends, &source)
}

fn capture_source(
    clipboard_owner: Option<isize>,
) -> Result<CaptureSource, SelectionCaptureFailure> {
    let foreground_window = foreground_window()?;
    Ok(CaptureSource {
        foreground_window,
        clipboard_owner: clipboard_owner.map(|hwnd| HWND(hwnd as *mut _)),
        source_process: process_name(foreground_window),
        source_window_title: window_title(foreground_window),
    })
}

fn run_capture_backends(
    backends: &mut [Box<dyn CaptureBackend>],
    source: &CaptureSource,
) -> CaptureResult {
    let mut last_failure = SelectionCaptureFailure {
        error: SelectionCaptureError::TextPatternUnavailable,
        capture_method: None,
        source_process: source.source_process.clone(),
        source_window_title: source.source_window_title.clone(),
    };

    for backend in backends {
        match backend.capture(source) {
            Ok(capture) => return finalize_backend_capture(source, capture),
            Err(BackendCaptureError::Recoverable(error)) => {
                last_failure = SelectionCaptureFailure {
                    error,
                    capture_method: Some(backend.name()),
                    source_process: source.source_process.clone(),
                    source_window_title: source.source_window_title.clone(),
                };
            }
            Err(BackendCaptureError::Fatal(error)) => {
                return Err(SelectionCaptureFailure {
                    error,
                    capture_method: Some(backend.name()),
                    source_process: source.source_process.clone(),
                    source_window_title: source.source_window_title.clone(),
                });
            }
        }
    }

    Err(last_failure)
}

fn finalize_backend_capture(source: &CaptureSource, capture: BackendCapture) -> CaptureResult {
    let text = normalize_line_endings(&capture.text);
    if text.is_empty() {
        return Err(SelectionCaptureFailure {
            error: SelectionCaptureError::EmptySelection,
            capture_method: Some(capture.capture_method),
            source_process: source.source_process.clone(),
            source_window_title: source.source_window_title.clone(),
        });
    }

    Ok(CaptureSuccess {
        selection: CapturedSelection {
            text,
            source_process: source.source_process.clone(),
            source_window_title: source.source_window_title.clone(),
            capture_method: capture.capture_method,
        },
    })
}

fn foreground_window() -> Result<HWND, SelectionCaptureFailure> {
    let hwnd = unsafe { GetForegroundWindow() };
    if hwnd.0.is_null() {
        Err(SelectionCaptureFailure {
            error: SelectionCaptureError::NoForegroundWindow,
            capture_method: None,
            source_process: None,
            source_window_title: None,
        })
    } else {
        Ok(hwnd)
    }
}

fn run_uia_selection_strategies(
    context: &CaptureContext<'_>,
) -> Result<(&'static str, String), SelectionCaptureError> {
    let mut last_error = SelectionCaptureError::TextPatternUnavailable;

    for strategy in selection_strategies() {
        match strategy.capture(context) {
            Ok(text) if text.is_empty() => {
                last_error = SelectionCaptureError::EmptySelection;
            }
            Ok(text) => return Ok((strategy.name(), text)),
            Err(error) => {
                last_error = error;
            }
        }
    }

    Err(last_error)
}

struct ClipboardGuard;

impl ClipboardGuard {
    fn open(owner: Option<HWND>) -> Result<Self, SelectionCaptureError> {
        unsafe {
            OpenClipboard(owner.unwrap_or_default()).map_err(map_windows_error)?;
        }
        Ok(Self)
    }
}

impl Drop for ClipboardGuard {
    fn drop(&mut self) {
        unsafe {
            let _ = CloseClipboard();
        }
    }
}

struct ClipboardBackup {
    data_object: Option<IDataObject>,
}

impl ClipboardBackup {
    fn capture() -> Self {
        Self {
            data_object: unsafe { OleGetClipboard().ok() },
        }
    }

    fn restore(&self, owner: Option<HWND>) -> Result<(), SelectionCaptureError> {
        if let Some(data_object) = &self.data_object {
            return retry_until(CLIPBOARD_RESTORE_TIMEOUT, || unsafe {
                OleSetClipboard(data_object).map_err(map_windows_error)
            });
        }

        retry_until(CLIPBOARD_RESTORE_TIMEOUT, || {
            let _clipboard = ClipboardGuard::open(owner)?;
            unsafe { EmptyClipboard().map_err(map_windows_error) }
        })
    }
}

fn retry_until<T>(
    timeout: Duration,
    mut operation: impl FnMut() -> Result<T, SelectionCaptureError>,
) -> Result<T, SelectionCaptureError> {
    let deadline = Instant::now() + timeout;
    let mut last_error: SelectionCaptureError;

    loop {
        match operation() {
            Ok(value) => return Ok(value),
            Err(error) => last_error = error,
        }

        if Instant::now() >= deadline {
            return Err(last_error);
        }
        thread::sleep(POLL_INTERVAL);
    }
}

fn capture_text_by_clipboard_copy(owner: Option<HWND>) -> Result<String, BackendCaptureError> {
    let backup = ClipboardBackup::capture();
    {
        let _clipboard = ClipboardGuard::open(owner)?;
        unsafe {
            EmptyClipboard().map_err(map_windows_error)?;
        }
    }

    let capture_result = (|| {
        let start_sequence = unsafe { GetClipboardSequenceNumber() };
        send_ctrl_c()?;
        wait_for_clipboard_text(start_sequence, CLIPBOARD_COPY_TIMEOUT)
    })();

    let restore_result = backup.restore(owner);
    match (capture_result, restore_result) {
        (Ok(text), Ok(())) => Ok(text),
        (Err(error), Ok(())) => Err(BackendCaptureError::Recoverable(error)),
        (Ok(_), Err(error)) | (Err(_), Err(error)) => Err(BackendCaptureError::Fatal(error)),
    }
}

fn key_is_down(key: VIRTUAL_KEY) -> bool {
    unsafe { GetAsyncKeyState(key.0 as i32) < 0 }
}

fn send_ctrl_c() -> Result<(), SelectionCaptureError> {
    let mut inputs = Vec::new();
    release_interfering_copy_modifiers(&mut inputs);
    inputs.extend([
        keyboard_input(VK_CONTROL, false),
        keyboard_input(VK_C, false),
        keyboard_input(VK_C, true),
        keyboard_input(VK_CONTROL, true),
    ]);
    let sent = unsafe { SendInput(&inputs, size_of::<INPUT>() as i32) };
    if sent == inputs.len() as u32 {
        Ok(())
    } else {
        Err(SelectionCaptureError::WindowsApiFailure(format!(
            "SendInput sent {sent} of {} keyboard events",
            inputs.len()
        )))
    }
}

fn release_interfering_copy_modifiers(inputs: &mut Vec<INPUT>) {
    for key in [VK_SHIFT, VK_MENU, VK_LWIN, VK_RWIN] {
        if key_is_down(key) {
            inputs.push(keyboard_input(key, true));
        }
    }
}

fn keyboard_input(key: VIRTUAL_KEY, key_up: bool) -> INPUT {
    INPUT {
        r#type: INPUT_KEYBOARD,
        Anonymous: INPUT_0 {
            ki: KEYBDINPUT {
                wVk: key,
                wScan: 0,
                dwFlags: if key_up {
                    KEYEVENTF_KEYUP
                } else {
                    Default::default()
                },
                time: 0,
                dwExtraInfo: 0,
            },
        },
    }
}

fn wait_for_clipboard_text(
    start_sequence: u32,
    timeout: Duration,
) -> Result<String, SelectionCaptureError> {
    let deadline = Instant::now() + timeout;
    let mut last_error = SelectionCaptureError::EmptySelection;

    while Instant::now() < deadline {
        let sequence_changed = unsafe { GetClipboardSequenceNumber() } != start_sequence;
        if sequence_changed {
            match read_clipboard_unicode_text() {
                Ok(text) => return Ok(text),
                Err(error) => last_error = error,
            }
        }
        thread::sleep(POLL_INTERVAL);
    }

    Err(last_error)
}

fn read_clipboard_unicode_text() -> Result<String, SelectionCaptureError> {
    let _clipboard = ClipboardGuard::open(None)?;
    read_clipboard_unicode_text_open_clipboard()
}

fn read_clipboard_unicode_text_open_clipboard() -> Result<String, SelectionCaptureError> {
    unsafe {
        IsClipboardFormatAvailable(CF_UNICODETEXT_FORMAT).map_err(|_| {
            SelectionCaptureError::WindowsApiFailure(
                "clipboard did not contain Unicode text".to_string(),
            )
        })?;
    }

    let handle = unsafe { GetClipboardData(CF_UNICODETEXT_FORMAT).map_err(map_windows_error)? };
    let hglobal = HGLOBAL(handle.0);
    let byte_len = unsafe { GlobalSize(hglobal) };
    if byte_len < 2 {
        return Err(SelectionCaptureError::EmptySelection);
    }

    let bytes = copy_global_memory(hglobal, byte_len)?;
    let words: Vec<u16> = bytes
        .chunks_exact(2)
        .map(|chunk| u16::from_le_bytes([chunk[0], chunk[1]]))
        .take_while(|word| *word != 0)
        .collect();
    if words.is_empty() {
        Err(SelectionCaptureError::EmptySelection)
    } else {
        Ok(String::from_utf16_lossy(&words))
    }
}

fn copy_global_memory(hglobal: HGLOBAL, size: usize) -> Result<Vec<u8>, SelectionCaptureError> {
    let pointer = unsafe { GlobalLock(hglobal) };
    if pointer.is_null() {
        return Err(SelectionCaptureError::WindowsApiFailure(
            "GlobalLock failed for clipboard data".to_string(),
        ));
    }

    let data = unsafe { std::slice::from_raw_parts(pointer.cast::<u8>(), size).to_vec() };
    let _ = unsafe { GlobalUnlock(hglobal) };
    Ok(data)
}

fn selected_text_from_element(
    automation: &IUIAutomation,
    element: &IUIAutomationElement,
) -> Result<String, SelectionCaptureError> {
    let mut saw_text_pattern = false;
    let mut last_error = SelectionCaptureError::TextPatternUnavailable;

    if let Ok(pattern) = current_text_pattern(element) {
        saw_text_pattern = true;
        match selected_text(&pattern) {
            Ok(text) => return Ok(text),
            Err(SelectionCaptureError::EmptySelection) => {
                last_error = SelectionCaptureError::EmptySelection;
            }
            Err(error) => last_error = error,
        }
    }

    for pattern in descendant_text_patterns(automation, element) {
        saw_text_pattern = true;
        match selected_text(&pattern) {
            Ok(text) => return Ok(text),
            Err(SelectionCaptureError::EmptySelection) => {
                last_error = SelectionCaptureError::EmptySelection;
            }
            Err(error) => last_error = error,
        }
    }

    if saw_text_pattern {
        Err(last_error)
    } else {
        Err(SelectionCaptureError::TextPatternUnavailable)
    }
}

fn descendant_text_patterns(
    automation: &IUIAutomation,
    element: &IUIAutomationElement,
) -> Vec<IUIAutomationTextPattern> {
    let mut patterns = Vec::new();

    if let Ok(descendants) = text_pattern_descendants(automation, element) {
        if let Ok(length) = unsafe { descendants.Length().map_err(map_windows_error) } {
            let capped_length = length.min(64);

            for index in 0..capped_length {
                let Ok(candidate) = (unsafe { descendants.GetElement(index) }) else {
                    continue;
                };
                if let Ok(pattern) = current_text_pattern(&candidate) {
                    patterns.push(pattern);
                }
            }
        }
    }

    patterns
}

fn text_pattern_descendants(
    automation: &IUIAutomation,
    element: &IUIAutomationElement,
) -> Result<IUIAutomationElementArray, SelectionCaptureError> {
    let condition = text_pattern_available_condition(automation)?;
    unsafe {
        element
            .FindAll(TreeScope_Descendants, &condition)
            .map_err(map_windows_error)
    }
}

fn current_text_pattern(
    element: &IUIAutomationElement,
) -> Result<IUIAutomationTextPattern, SelectionCaptureError> {
    let pattern = unsafe {
        element
            .GetCurrentPattern(UIA_TextPatternId)
            .map_err(|_| SelectionCaptureError::TextPatternUnavailable)?
    };

    pattern
        .cast::<IUIAutomationTextPattern>()
        .map_err(|_| SelectionCaptureError::TextPatternUnavailable)
}

fn text_pattern_available_condition(
    automation: &IUIAutomation,
) -> Result<windows::Win32::UI::Accessibility::IUIAutomationCondition, SelectionCaptureError> {
    unsafe {
        let value = VARIANT::from(true);
        automation
            .CreatePropertyCondition(UIA_IsTextPatternAvailablePropertyId, &value)
            .map_err(map_windows_error)
    }
}

fn selected_text(pattern: &IUIAutomationTextPattern) -> Result<String, SelectionCaptureError> {
    let ranges = unsafe {
        pattern
            .GetSelection()
            .map_err(|error| classify_windows_error(error.code()))?
    };

    let length = range_count(&ranges)?;
    if length == 0 {
        return Err(SelectionCaptureError::EmptySelection);
    }

    let mut parts = Vec::new();
    for index in 0..length {
        let range = unsafe { ranges.GetElement(index).map_err(map_windows_error)? };
        let text: BSTR = unsafe { range.GetText(-1).map_err(map_windows_error)? };
        let normalized = normalize_line_endings(&text.to_string());
        if !normalized.is_empty() {
            parts.push(normalized);
        }
    }

    let joined = parts.join("\n");
    if joined.is_empty() {
        Err(SelectionCaptureError::EmptySelection)
    } else {
        Ok(joined)
    }
}

fn range_count(ranges: &IUIAutomationTextRangeArray) -> Result<i32, SelectionCaptureError> {
    unsafe { ranges.Length().map_err(map_windows_error) }
}

fn window_title(hwnd: HWND) -> Option<String> {
    let length = unsafe { GetWindowTextLengthW(hwnd) };
    if length <= 0 {
        return None;
    }

    let mut buffer = vec![0u16; length as usize + 1];
    let copied = unsafe { GetWindowTextW(hwnd, &mut buffer) };
    if copied <= 0 {
        return None;
    }

    Some(String::from_utf16_lossy(&buffer[..copied as usize]))
}

fn process_name(hwnd: HWND) -> Option<String> {
    let mut process_id = 0u32;
    unsafe {
        GetWindowThreadProcessId(hwnd, Some(&mut process_id));
    }
    if process_id == 0 {
        return None;
    }

    let handle = unsafe {
        OpenProcess(
            PROCESS_QUERY_LIMITED_INFORMATION | PROCESS_VM_READ,
            false,
            process_id,
        )
        .ok()?
    };

    let mut buffer = vec![0u16; MAX_PATH as usize];
    let copied = unsafe { K32GetModuleBaseNameW(handle, None, &mut buffer) };
    unsafe {
        let _ = CloseHandle(handle);
    }

    if copied == 0 {
        None
    } else {
        Some(
            OsString::from_wide(&buffer[..copied as usize])
                .to_string_lossy()
                .into_owned(),
        )
    }
}

fn classify_windows_error(code: windows::core::HRESULT) -> SelectionCaptureError {
    if code.0 == windows::Win32::Foundation::E_ACCESSDENIED.0 {
        SelectionCaptureError::AccessDenied
    } else {
        SelectionCaptureError::WindowsApiFailure(format!("HRESULT 0x{:08X}", code.0 as u32))
    }
}

fn map_windows_error(error: windows::core::Error) -> SelectionCaptureError {
    classify_windows_error(error.code())
}

#[cfg(test)]
mod tests {
    use super::{
        finalize_backend_capture, BackendCapture, CaptureSource, SelectionCaptureError, HWND,
    };

    fn test_source() -> CaptureSource {
        CaptureSource {
            foreground_window: HWND::default(),
            clipboard_owner: None,
            source_process: Some("example.exe".to_string()),
            source_window_title: Some("Example".to_string()),
        }
    }

    #[test]
    fn finalizes_clipboard_and_uia_captures_with_identical_text_rules() {
        for method in ["clipboard-copy", "uia-focused-element"] {
            let result = finalize_backend_capture(
                &test_source(),
                BackendCapture {
                    capture_method: method,
                    text: "one\r\ntwo\rthree".to_string(),
                },
            )
            .expect("capture should finalize");

            assert_eq!(result.selection.capture_method, method);
            assert_eq!(result.selection.text, "one\ntwo\nthree");
            assert_eq!(
                result.selection.source_process,
                Some("example.exe".to_string())
            );
            assert_eq!(
                result.selection.source_window_title,
                Some("Example".to_string())
            );
        }
    }

    #[test]
    fn finalizes_clipboard_and_uia_empty_captures_as_empty_selection() {
        for method in ["clipboard-copy", "uia-focused-element"] {
            let failure = match finalize_backend_capture(
                &test_source(),
                BackendCapture {
                    capture_method: method,
                    text: String::new(),
                },
            ) {
                Ok(_) => panic!("empty capture should fail consistently"),
                Err(failure) => failure,
            };

            assert!(matches!(
                failure.error,
                SelectionCaptureError::EmptySelection
            ));
            assert_eq!(failure.capture_method, Some(method));
        }
    }
}
