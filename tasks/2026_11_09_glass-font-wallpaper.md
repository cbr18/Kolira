# Glass: обои и KDE шрифт

Статус: DONE, 2026-11-09 (follow-up live validation).

## Причина

Последний успешный Apply до исправления был профилем `dracula`, который сам
выбирает Look and Feel `glass`; профиль `glass` тогда ещё не применялся.
Предпросмотр профиля `glass` показывал Saturn обои вместо текущих Dracula.
В KDE шрифте сохранялся `activeFont=Monocraft`: ключ отсутствовал в разрешённом
списке appearance keys для `kdeglobals`, хотя это семейство шрифта темы.

## Исправление и результат

`activeFont` добавлен в обрабатываемые appearance keys. Если в целевой теме
его нет, старый Minecraft шрифт удаляется. Также исправлена синхронизация GTK:
при Apply `font-name` получает семейство из темы с сохранением размера, а
устаревший `monospace-font-name` переходит на системный Monospace с сохранением
размера. Apply записывает исходные GSettings для Undo. Документация обновлена.

Профиль `glass` применён штатным Rust CLI. Операция `apply-L1Tvp6` завершилась
успешно, ошибок в отчёте нет; резервная копия:
`~/.config/kolira/backups/pre-apply-glass-90484-1794206853240837894-0`.
На обоих активных рабочих столах выбран `glass-saturn-mono.jpg`, целевой файл
существует. Ключ `activeFont=Monocraft` в текущем `kdeglobals` отсутствует,
GTK и Qt используют Noto Sans. CLI собран и установлен штатным установщиком;
бинарный бэкап — `~/.config/kolira/backups/deploy-kolira-20261109-164840-90957`.

Повторное применение Glass: `apply-mVKTlD`, ошибок нет; резервная копия:
`~/.config/kolira/backups/pre-apply-glass-120126-1794216593501053986-0`.
Живые запросы Plasma для обоих экранов сообщают wallpaper `glass-saturn-mono.jpg`;
после этого `plasma-apply-wallpaperimage` повторно назначил его обоим рабочим
столам. Рядом оставалась неактивная запись лесного слайд-шоу; текущий plugin
`org.kde.image` и его живые параметры показывают Saturn и `SlideshowMode=0`.
Файл в `$HOME/Изображения/Wallpaper/` существует. GSettings после Apply:
`font-name='Noto Sans, 10'`, `monospace-font-name='Monospace 10'`.
Повторная установка CLI: `~/.config/kolira/backups/deploy-kolira-20261109-194308-128972`.
Прогнаны `cargo test` и unit-тест размеров/семейств GTK-шрифтов; Kvantum
`OrchisDark` не установлен (предупреждение, основной Apply успешен).

Follow-up 2026-11-10: автоматический сброс monospace отменён; независимый
пользовательский шрифт сохраняется, включая Undo старых журналов. См.
[исправление Minecraft и Zen](2026_11_10_minecraft-followup.md).
