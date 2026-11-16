# Starship, tmux, Foot, WezTerm

Статус: IN PROGRESS — 2026-11-16. Код и ресурсы реализованы; завершается
усиленная native-проверка WezTerm. Установленная копия не обновлялась.

## Результат

- Четыре новых app_targets и записи реестра: 53 приложения. Все восемь
  комплектных тем обновлены из сохранённой base16/accent-палитры: по 35 targets.
- Starship: palette/собственная таблица цветов; модули, команды, format и shell
  init сохраняются. Цвета видны на следующем prompt.
- tmux: только global foreground/background восьми style options. Работающие
  серверы обновляются без остановки сессий; commands и атрибуты стиля сохраняются.
  JSON содержит только RGB-данные, startup script генерирует Rust core.
- Foot: native ANSI, фон/текст, выделение и курсор; шрифты, alpha, команды и
  bindings остаются личными. Для открытых окон/server mode допустим перезапуск.
- WezTerm: RGB/TOML + фиксированное подключение к существующему Lua. Скрипты из
  темы игнорируются. Меняются только девять управляемых полей; native defaults
  не должны затирать личные indexed colours и оформление вкладок.

## Защита и возврат

Все изменения проходят общий Plan/preview, MutationGuard и safety transaction.
Возврат к теме без конфига восстанавливает baseline, включая отсутствие файлов,
с сохранением поздних личных правок. Lua и tmux commands не переносятся в пакеты.
Конфликт с личным script/loader и неподдерживаемые env/config paths останавливают
preflight до записи. Проверены XDG relocation и порядок выбора WezTerm config.

Native tmux values имеют before/after в apply-journal и safety snapshots, включая
socket inode/device и PID сервера. Возврат сохраняет поздние цвета/атрибуты;
старый журнал не меняет заменивший сервер на том же socket path. Серверы не
создаются и не завершаются в Apply. NO_APPLY отключает version/runtime commands.

Версии: Starship 1.27+, tmux 3.2+, Foot 1.28+, WezTerm build 20220807+.
Старые версии отклоняются до изменения конфигов. Подробные ограничения:
[APPLICATIONS.md](../docs/APPLICATIONS.md#starship-tmux-foot-wezterm).

## Проверка

- Общий Rust-прогон: **287 PASS**, включая десять регрессий новой партии.
  После усиления проверки RGB повторно пройдены десять регрессий и три проверки
  maintenance refresh. Clippy -D warnings и release build PASS.
- **345 Python/Qt**, **260 shell**, shellcheck, cargo fmt, Python syntax PASS.
  Переводы: **920 finished, 0 unfinished**.
- Восемь комплектов: SHA-256 сходятся; прежние KDE ColorScheme и wallpaper-accent
  choices сохранены. Выделение/курсор и tmux text pairs проверены на контраст.
- Native tmux 3.8 от UID 1000: восемь Apply/возвратов и искусственных сбоев PASS;
  личные цвета, поздние атрибуты и replacement server сохраняются.
- Native Starship RGB и Foot parser: восемь тем PASS. WezTerm с простым config
  прошёл первоначальный smoke. Усиленный fixture с config_builder/вкладками
  потребовал исправления: в active_tab отсутствовал обязательный bg_color.
  Проверка теперь учитывает stderr, поскольку ls-fonts может вернуть exit 0 при
  ошибке config. Итоговый повтор с корректным fixture выполняется.

Протоколы: /tmp/kolira-four-rust-complete.log, /tmp/kolira-four-python.log,
/tmp/kolira-four-shell.log, /tmp/kolira-four-native-user.log. Native тестируется в
одноразовом Docker без сети и host HOME; репозиторий смонтирован read-only.
В tmux rollback fixture заглушён только systemctl: реальной Plasma в этом образе
нет. Полный живой KWin/GUI-цикл этой партии не заявляется.

## Границы поставки

Текущие темы рабочего стола и личные приложения не применялись/не перезапускались.
Изменения подготовлены в репозитории; live deployment и новый релиз не выполнялись.
После завершения этой партии следующие новые P1 — Rofi, Fuzzel и Walker.
