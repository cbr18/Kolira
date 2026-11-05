# Автоматизация через MCP

**Статус:** DONE (2026-11-05): get_automation, set_automation, list_actions, run_action; apply_profile отдаёт dependencies и actions; описания инструментов требуют согласия человека. Продолжение
`2026_11_05_21_00_automate-manual-steps.md`.

## Что сделать

Локальный MCP-сервер (`mcp/theme_profile_mcp.py`, `docs/MCP.md`) должен уметь то
же, что GUI и CLI, — с теми же правилами согласия:

- `get_automation` / `set_automation` — выбор пользователя (`theme-profile
  automation --json` / `set KEY=ask|always|never`); менять только по явной
  просьбе пользователя, не «для удобства».
- `list_actions(theme)` — что осталось после применения (`theme-profile actions
  NAME --json`): перезапуски, подсказка Telegram, с предупреждениями.
- `run_action(id)` — выполнить одно действие (`theme-profile action ID`);
  в описании инструмента — предупреждение (звонок в Vesktop/Zen прервётся).
- `install_dependencies` уже есть — описать, что системные пакеты открывают
  терминал и ждут пароль пользователя (MCP-клиент не должен считать это
  «установлено»).
- `apply_theme` — отдавать `actions` и учитывать `automation` (always/never
  уже применяются ядром; `ask` — вернуть списком, чтобы ассистент спросил
  пользователя).

## Безопасность

- Ничего из `ask` не выполнять без подтверждения человека.
- Никогда не запускать sudo; пакеты — только через терминал пользователя.
- Тесты: всё под `KTM_THEME_NO_APPLY=1`, временный HOME.
