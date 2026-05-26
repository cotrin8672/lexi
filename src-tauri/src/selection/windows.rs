use crate::selection::{
    error_code, normalize_line_endings, CapturedSelection, SelectionCaptureError,
    SelectionCaptureFailure, SelectionDiagnostics,
};
use std::ffi::OsString;
use std::os::windows::ffi::OsStringExt;
use windows::core::{Interface, BSTR, VARIANT};
use windows::Win32::Foundation::{CloseHandle, HWND, MAX_PATH};
use windows::Win32::System::Com::{
    CoCreateInstance, CoInitializeEx, CoUninitialize, CLSCTX_INPROC_SERVER,
    COINIT_APARTMENTTHREADED,
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
use windows::Win32::UI::WindowsAndMessaging::{
    GetForegroundWindow, GetWindowTextLengthW, GetWindowTextW, GetWindowThreadProcessId,
};

struct ComApartment;

impl ComApartment {
    fn init() -> Result<Self, SelectionCaptureError> {
        unsafe {
            CoInitializeEx(None, COINIT_APARTMENTTHREADED)
                .ok()
                .map_err(map_windows_error)?;
        }
        Ok(Self)
    }
}

impl Drop for ComApartment {
    fn drop(&mut self) {
        unsafe {
            CoUninitialize();
        }
    }
}

pub fn capture_selected_text() -> Result<CapturedSelection, SelectionCaptureError> {
    capture_selected_text_with_context()
        .map(|result| result.selection)
        .map_err(|failure| failure.error)
}

pub fn capture_selected_text_with_failure() -> Result<CapturedSelection, SelectionCaptureFailure> {
    capture_selected_text_with_context().map(|result| result.selection)
}

pub fn capture_selection_diagnostics() -> SelectionDiagnostics {
    match capture_selected_text_with_context() {
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

struct CaptureContext {
    automation: IUIAutomation,
    focused_element: IUIAutomationElement,
    foreground_element: IUIAutomationElement,
    source_process: Option<String>,
    source_window_title: Option<String>,
}

trait SelectionStrategy {
    fn name(&self) -> &'static str;

    fn capture(&self, context: &CaptureContext) -> Result<String, SelectionCaptureError>;
}

struct FocusedElementStrategy;

impl SelectionStrategy for FocusedElementStrategy {
    fn name(&self) -> &'static str {
        "uia-focused-element"
    }

    fn capture(&self, context: &CaptureContext) -> Result<String, SelectionCaptureError> {
        selected_text_from_element(&context.automation, &context.focused_element)
    }
}

struct ForegroundWindowStrategy;

impl SelectionStrategy for ForegroundWindowStrategy {
    fn name(&self) -> &'static str {
        "uia-foreground-window"
    }

    fn capture(&self, context: &CaptureContext) -> Result<String, SelectionCaptureError> {
        selected_text_from_element(&context.automation, &context.foreground_element)
    }
}

fn selection_strategies() -> [Box<dyn SelectionStrategy>; 2] {
    [
        Box::new(FocusedElementStrategy),
        Box::new(ForegroundWindowStrategy),
    ]
}

fn capture_selected_text_with_context() -> Result<CaptureSuccess, SelectionCaptureFailure> {
    let foreground = foreground_window()?;
    let source_window_title = window_title(foreground);
    let source_process = process_name(foreground);

    let _com = ComApartment::init().map_err(|error| SelectionCaptureFailure {
        error,
        capture_method: None,
        source_process: source_process.clone(),
        source_window_title: source_window_title.clone(),
    })?;
    let automation: IUIAutomation = unsafe {
        CoCreateInstance(&CUIAutomation, None, CLSCTX_INPROC_SERVER).map_err(|error| {
            SelectionCaptureFailure {
                error: map_windows_error(error),
                capture_method: None,
                source_process: source_process.clone(),
                source_window_title: source_window_title.clone(),
            }
        })?
    };

    let focused = unsafe {
        automation.GetFocusedElement().map_err(|error| {
            let error = match classify_windows_error(error.code()) {
                SelectionCaptureError::AccessDenied => SelectionCaptureError::AccessDenied,
                SelectionCaptureError::WindowsApiFailure(_) => {
                    SelectionCaptureError::FocusedElementUnavailable
                }
                other => other,
            };
            SelectionCaptureFailure {
                error,
                capture_method: None,
                source_process: source_process.clone(),
                source_window_title: source_window_title.clone(),
            }
        })?
    };
    let foreground_element = unsafe {
        automation
            .ElementFromHandle(foreground)
            .map_err(|error| SelectionCaptureFailure {
                error: map_windows_error(error),
                capture_method: None,
                source_process: source_process.clone(),
                source_window_title: source_window_title.clone(),
            })?
    };

    let context = CaptureContext {
        automation,
        focused_element: focused,
        foreground_element,
        source_process,
        source_window_title,
    };

    let (capture_method, text) = run_selection_strategies(&context)?;

    Ok(CaptureSuccess {
        selection: CapturedSelection {
            text,
            source_process: context.source_process,
            source_window_title: context.source_window_title,
            capture_method,
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

fn run_selection_strategies(
    context: &CaptureContext,
) -> Result<(&'static str, String), SelectionCaptureFailure> {
    let mut last_failure = SelectionCaptureFailure {
        error: SelectionCaptureError::TextPatternUnavailable,
        capture_method: None,
        source_process: context.source_process.clone(),
        source_window_title: context.source_window_title.clone(),
    };

    for strategy in selection_strategies() {
        match strategy.capture(context) {
            Ok(text) if text.is_empty() => {
                last_failure = SelectionCaptureFailure {
                    error: SelectionCaptureError::EmptySelection,
                    capture_method: Some(strategy.name()),
                    source_process: context.source_process.clone(),
                    source_window_title: context.source_window_title.clone(),
                };
            }
            Ok(text) => return Ok((strategy.name(), text)),
            Err(error) => {
                last_failure = SelectionCaptureFailure {
                    error,
                    capture_method: Some(strategy.name()),
                    source_process: context.source_process.clone(),
                    source_window_title: context.source_window_title.clone(),
                };
            }
        }
    }

    Err(last_failure)
}

fn selected_text_from_element(
    automation: &IUIAutomation,
    element: &IUIAutomationElement,
) -> Result<String, SelectionCaptureError> {
    let mut saw_text_pattern = false;
    let mut last_error = SelectionCaptureError::TextPatternUnavailable;

    for pattern in text_patterns_for_element(automation, element) {
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

fn text_patterns_for_element(
    automation: &IUIAutomation,
    element: &IUIAutomationElement,
) -> Vec<IUIAutomationTextPattern> {
    let mut patterns = Vec::new();

    if let Ok(pattern) = current_text_pattern(element) {
        patterns.push(pattern);
    }

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
