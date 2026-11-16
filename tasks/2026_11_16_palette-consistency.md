# Палитра: согласованность восьми тем и акцент браузеров

Статус: DONE — 2026-11-16.

Пользователь обнаружил красный Firefox при Everforest. В repo и установленном
пакете GTK Breeze colors.css содержал красно-розовую схему, не Everforest;
Firefox использовал system theme, комплектный snapshot задавал лишь day/night.
Прежние проверки возврата/контраста не доказывали соответствие палитре.

Контракт браузеров уточнён пользователем: только native accent из palette.accent
и day/night; не переписывать кнопки, надписи, иконки, layout и userChrome.
Используются native ui.accentcolor/ui.accentcolortext (Mozilla LookAndFeel) и
zen.theme.accent-color (Zen); stylesheet flag не включается. Старые управляемые
browser CSS больше не перекрашивают chrome. prefs.js не редактируется.

Проверить восемь комплектных/локальных тем, native selectors и активные ресурсы
всех поддержанных групп; устранить stale GTK/terminal/appearance data. Проверять
не только запись и возврат, но соответствие фактических native значений палитре.
Указать реальные визуальные проверки отдельно от анализа файлов/тестов.

Источники native browser prefs:
- https://searchfox.org/firefox-main/source/widget/nsXPLookAndFeel.cpp
- https://docs.zen-browser.app/guides/about-config-flags


## Реализовано и проверено

- Обновлены восемь repo и локальных пакетов. GTK/Breeze named definitions,
  активные KDE scheme/colour fields, Alacritty/Ghostty, OMP и обе партии app
  resources сверяются с палитрой. Native browser accent/day-night больше не
  использует CSS overrides для toolbar, кнопок, подписей или иконок.
- Расширенный тест всех восьми пакетов проверяет значения **и выбор** ресурсов
  Cursor/Code, Zed, Neovim, Obsidian, Discord/Vesktop, 
  Kitty/Alacritty/Ghostty/Konsole, shell/TUI и AI selectors, затем возврат.
- Полный Rust: 263 PASS; расширенная матрица/возврат 16 PASS; дополнительная
  регрессия GTK active foreground/alpha PASS. Clippy all-targets -D warnings PASS;
  release build PASS. После Docker snapshot менялись только дополнительные тесты и запрет maintenance-
  helper писать напрямую в live themes; Rust core совпадает с Docker snapshot
  и установленным release binary.
- GUI/Python: 345 PASS. Shell/installer: 260 PASS. shellcheck, qmllint,
  Python syntax, diff whitespace PASS. Переводимые строки не добавлялись.
- Настоящие Firefox и Zen в disposable HOME: baseline → все восемь тем → neutral,
  native accent и day/night совпадают, stock icons контрастны, отключённый CSS
  остаётся отключённым; исходные accent/system colours возвращаются. PASS.
  Логи: `/tmp/kolira-native-firefox-accent-final.log`,
  `/tmp/kolira-native-zen-accent-final.log`.
- Native GTK parser: 19 CSS files, 0 errors. 76 foreground/background pairs,
  минимальный contrast 5.51:1; проверены selection, menu/button, header, view/window.
  Логи `/tmp/kolira-palette-consistency-gtk.json`, `/tmp/kolira-gtk-colour-pairs.json`.
- Повторный refresh repo и локальных staging пакетов побайтно идентичен.
- Docker: восемь тем в реальной Plasma, возврат файлов/GSettings, живые KWin,
  plasmashell/GUI и возврат через app controller — ALL PASSED. Exit 0.
  `/tmp/kolira-palette-consistency-docker.log`,
  `/tmp/kolira-palette-consistency-e2e-out/`. Core snapshot соответствует текущему core.
- Maintenance helper проверен на отказ писать в live themes до чтения/записи
  пакетов; повторный refresh/idempotence test PASS. Helper не развёртывается как
  приложение и используется только на repo/staging, затем CLI import с backup.

## Установлено и применено

- CLI/GUI обновлены штатным installer, GUI перезапущен. Все восемь локальных
  пакетов обновлены через import --force с pre-import backups; сохранены обои,
  настройки формы и прозрачности. Preview каждого пакета на реальных файлах PASS.
- Применена активная **everforest**, без переключения на другие темы и с
  keep_wallpaper=true. Операция `apply-hKWDh8`: ошибок/пропусков/missing нет.
- Backup: `~/.config/kolira/backups/pre-apply-everforest-1381273-1794781214731769853-0`.
  Deploy backup: `~/.config/kolira/backups/deploy-kolira-20261116-081948`.
- Артефакты: `/tmp/kolira-palette-consistency-u2wbkmvu/` (import-results.json,
  live-previews.json, apply-result.json, installed-consistency.json).
- Live KDE AccentColor=167,192,128; ColorScheme=KoliraEverforest.
  GTK3/4 selection и native prefs Firefox/Zen: #a7c080.
  В трёх обнаруженных браузерных профилях нет kolira-colors.css/его import.
  KWin/Plasma отвечают на D-Bus Peer.Ping.
- Браузеры не закрывались, вкладки не затрагивались. Работающему браузеру может
  потребоваться штатный перезапуск для чтения user.js.

## Границы

[Аудит](../docs/audits/2026_11_16_palette-consistency.md) содержит инвентаризацию
всех 41 записей реестра и отделяет реальные native browser проверки от чтения
конфигов/селекторов других программ. Не объявляется визуальный тест каждого
окна каждого приложения. Явно выбранные Kvantum/Plasma/decorations/library assets
имеют собственные стили/SVG и не становятся base16 templates. Неустановленные
программы не устанавливались; очередные неподдержанные приложения не включались.
