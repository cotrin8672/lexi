# Design fixtures

UIデザイン用の未整形サンプルデータを置く場所です。

## `word-study-memo.raw.json`

語彙メモ画面を設計するときは、まず `providerStructuredResponse` を主な表示データとして見てください。これはLLMから返ってきた構造化レスポンスを、画面用に整形する前の形です。

デザイナーが優先して見せ方を考える情報は次の通りです。

- `headword`: 辞書形または見出し語。
- `translations`: 日本語訳、品詞、例文。
- `nuance`: その語をどういう場面で使うかの短い説明。
- `synonyms`: 類似語と、見出し語との使い分け。
- `warnings`: モデルの確信度が低い場合など、ユーザーに見せる注意。

周辺の `popup`、`capture`、`request`、`streamEvents`、`alternateStates` は、待機中、取得中、生成中、完了、エラー状態を設計するための補助情報です。

選択された原文は機密情報として扱うため、fixtureには保存しません。
