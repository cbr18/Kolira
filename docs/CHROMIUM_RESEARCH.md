# Исследование Chromium automatic accent (задача 2026_11_16)

**Вердикт:** отложено (P4), ветка `feat/chromium-auto-accent` удалена. Chromium не требует изменений в kolira на данный момент.

## Проблема

Применение managed-политики `BrowserThemeColor` меняет акцент Chromium без перезапуска, но при снятии политики `extensions.theme.system_theme` сбрасывается в `0` (default), теряя исходный выбор GTK (`1`) или Qt (`2`). Файловый backup политики не помогает — браузер уже изменил внутреннее состояние.

Прямая запись `Preferences` или системных политик запрещена контрактом kolira (не требуем root, не правим Preferences пользователя).

## Найденное решение (не реализовано)

В ходе экспериментов на хосте (Chromium 157.0.8103.38, CDP через WebSocket) подтверждено, что встроенный механизм браузера `chrome.send()` может переключать `extensions.theme.system_theme` в `Default/Preferences` без прямой записи файла:

| Команда | system_theme | Результат |
|---|---|---|
| `chrome.send("useGtkTheme")` | 2 (Qt) → 1 (GTK) | ✅ Работает |
| `chrome.send("useQtTheme")` | 1 (GTK) → 2 (Qt) | ✅ Работает |
| `chrome.send("useDefaultTheme")` | 1 (GTK) → 0 (Default) | ✅ Работает |

Задержка применения: ~5–10 с. Работает только на страницах `chrome://*` (не на обычных веб-страницах). WebUI `chrome://settings/appearance` через CDP не рендерит DOM, но `chrome.send` на ней выполняется.

## Почему не реализовано (пока)

1. **Требуется root** для записи managed-политики в `/etc/chromium/policies/managed/` — kolira сегодня не требует root от пользователя, и вводить это только для Chromium пока не решено.
2. **Требуется доступ к CDP** запущенного браузера (`--remote-debugging-port`) — не всегда удобно.
3. **Полный цикл не подтверждён:** Apply → политика → обновление браузера → Undo → `chrome.send` возвращает baseline. Эта цепочка не склеена в работающий поток.

Решение через `chrome.send()` найдено и подтверждено экспериментом, но само решение **пока не принято к реализации**. Что будет дальше — неизвестно, можно вернуться к задаче в будущем.

В Omarchy та же проблема решается через root + `sudo` для политики, но kolira пока не идёт по этому пути.

## Файлы исследования (удалены с веткой)

- `tests/e2e/chromium-chrome-send.py` — e2e-тест механизма `chrome.send` (все 3 перехода PASS)
- `tasks/2026_11_16_chromium-policy-sync.md` — обновлено с находками
- `docs/APP_COVERAGE_PRIORITIES.md` — строка Chromium в P4
- `docs/APPLICATIONS.md` — раздел про Chromium
- `gui/qt-app/src/core/rust_core/src/tracked_apps.rs` — описания обновлены

Если в будущем вернёмся к задаче: отправная точка — `chrome.send()` через CDP, не прямая запись Preferences.
