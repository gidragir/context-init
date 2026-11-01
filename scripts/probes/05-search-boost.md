# Probe 05: Search Boost

## Question
Какие веса (boost) назначены полям в поисковом индексе BM25 при ранжировании подсистем, и какой штрафной множитель применяется к тестовым подсистемам?

## Kind
pointed

## Expected
- `crates/ci-core/src/search.rs:111`: Массив `weighted` задает веса полей: `name`: 3.0, `symbols`: 3.0, `summary`: 2.0, `tags`: 1.5, `card`: 1.0, `files`: 1.0.
- `crates/ci-core/src/search.rs:86`: Константа `TESTS_TAG: &str = "tests";` и штрафной множитель `TESTS_SCORE_FACTOR: f32 = 0.5;` для уменьшения релевантности тестов.
- `crates/ci-core/src/search.rs:189`: Цикл применения весов к полям в Tantivy `QueryParser`: `parser.set_field_boost(*field, *boost);`.
- `crates/ci-core/src/search.rs:211`: Применение штрафа при наличии тега тестов: `if entry.tags.iter().any(|t| t == TESTS_TAG) { score * TESTS_SCORE_FACTOR }`.
