# MCP и GUI: точечная проверка ошибок и предпросмотра

Статус: DONE, 2026-11-10. Исправлены три найденные проблемы;
README/AGENTS v2 доставлены в пользовательский каталог тем.

## Исходные подтверждённые проблемы (исправлены)

1. P1 — ошибка profile.list превращается в пустой список. CliProfileCore.list_profiles
   подавляет ValueError/RuntimeError, ThemeBundleCore.profiles также возвращает [].
   MCP list_profiles выдаёт isError:false и profiles:[], а ProfileController.refresh
   трактует этот результат как исчезновение активного профиля. Rust повторно
   проверяет существование каталога и сохраняет маркер на диске, однако GUI
   безусловно очищает своё activeProfile. Результат: темы исчезают из списка,
   GUI теряет активный профиль при временном сбое чтения.
2. P1 — apply_profile в MCP не требует previewToken. Схема требует только name,
   обработчик передаёт None, Rust preview::confirmed выполняет операцию без
   проверки предпросмотра. В отличие от явно сохранённого прямого CLI-сценария,
   документация MCP описывает preview_profile → apply_profile с токеном.
3. P2 — ProfileController.refresh синхронно вызывает subprocess-backed
   list_profiles/get_active_profile/pending_operations в потоке Qt. Медленный
   core блокирует обработку ввода и отрисовку. Прогнозы summaries уже вынесены
   в worker, но основной refresh остаётся синхронным.

## Воспроизведение и пределы

Изолированная проверка /tmp/kolira-mcp-review.py создаёт отдельный HOME/XDG,
KTM_THEME_NO_APPLY=1 и KTM_OFFLINE=1; реальное оформление не менялось.

- Настоящий MCP/Rust prepare_theme и затем apply_profile(name=probe) без
  previewToken: isError:false, ok:true, изолированный kdeglobals изменился.
- Подставной CLI возвращает ошибку только profile.list, остальные запросы идут
  настоящему Rust: MCP возвращает profiles:[], isError:false.
- Тот же core в ProfileController: activeInGui="", activeOnDisk="probe".
- list_profiles с задержкой 300 мс: refresh занимает 300 мс, Qt singleShot(10)
  не обслуживается до его возврата.
- Штатные tests/test_mcp_workflow.py: 7 passed. Они не покрывают эти случаи.

Это точечная проверка Python MCP/GUI ↔ Rust. Полный аудит GUI/QML,
параллельных мутаций, внешних установщиков и всех MCP-инструментов не выполнен.

## Реализация и проверка

- CliProfileCore не подавляет ошибку profile.list; ThemeBundleCore не возвращает
  выдуманный пустой список. MCP отдаёт isError:true/core_unavailable.
- ProfileController.refresh выполняет чтение, проверку stale-маркера и истории
  в worker. Повторные запросы объединяются; модели Qt обновляются в главном
  потоке только после успешного чтения. При ошибке последняя корректная модель
  и activeProfile сохраняются. Отказ Rust очистить маркер больше не очищает GUI.
  Startup auto-apply ждёт окончания асинхронной загрузки.
- MCP требует previewToken в схеме и проверяет непустое значение до работы.
  Rust profile.apply при source=mcp требует preview_token до начала операции;
  прямой CLI без токена остаётся совместимым.
- agent.install v2 управляет README.md, AGENTS.md и CLAUDE.md: backup и сохранение
  пользовательских правок с .kit-new. README.md и README.md.kit-new зарезервированы
  как имена назначения темы без изменения синтаксического regex имён.
  Инструкции содержат offline prepare/clone, schema/field IDs, палитру,
  согласованность snapshot/metadata, зависимости, preview/apply и восстановление.

204 Rust-теста прошли, clippy --all-targets -D warnings чистый, release собран.
Python/Qt: 342 passed, 1 deselected. Исключён только прежний тест pipeline переводов:
в рабочем дереве уже удалён scripts/update-translations.sh. Удаление сохранено.
Single-instance проверен вне песочницы (внутри изоляция запрещает Qt socket).
Регрессии покрывают ошибку списка, сохранение GUI-модели, отказ stale clear,
неблокирующий/coalesced refresh, отсутствие/пустой previewToken и документы.

Полный штатный установщик выполнен с --no-themes --no-original-theme:
CLI backup ~/.config/kolira/backups/deploy-kolira-20261110-100334-58218,
GUI backup ~/.config/kolira/backups/deploy-kolira-20261110-100334.
Три управляемых документа получили state=updated. Деплоенные README/AGENTS
доступны в ~/.config/kolira/themes/.

Дополнительная проверка /tmp/kolira-installed-workflow.py в отдельном HOME/XDG
использовала установленный MCP и GUI из произвольного cwd: agent.install,
prepare из README, check/deps, clone, export/import, отклонение MCP Apply без
preview и успешный Apply с токеном, затем асинхронная загрузка активной темы GUI.
Пользовательское оформление этой проверкой не изменялось.
