# CONFIG NOT FOUND и blur Minecraft

Статус: DONE, 2026-11-10.

Причина prompt: fish продолжал ссылаться на kolira.omp.json после удаления
этого созданного темой файла в Undo. Сохранился исходный theme-profile.omp.json;
он скопирован обратно по ожидаемому пути. Undo должен сохранять общий prompt,
согласно уже действовавшему правилу plan_reset.

Minecraft: Better Blur DX по Glass (11/80 и остальные настройки эффекта),
Kvantum translucent/blur, терминалы 0.5, Klassy 50/48. Анимации остаются
отключёнными; пакетные метаданные и контрольные суммы обновляются вместе.

## Проверка и деплой

202 Rust-теста прошли, включая switching_to_theme_without_prompt_keeps_shell_config_target.
Release build, rustfmt, git diff --check и проверка 54 хешей пакета прошли.
Секция Better Blur DX в Minecraft совпадает с Glass. CLI установлен штатно,
пакет импортирован с backup:
~/.config/kolira/backups/pre-import-minecraft-overworld-dark-36534-1794266168141933414-0.

Apply apply-r90mjd успешен, errors/missing/skipped пусты; backup:
~/.config/kolira/backups/pre-apply-minecraft-overworld-dark-36608-1794266180015571157-0.
Живой KWin loadedEffects содержит better_blur_dx; Kvantum translucent/blurring
включены, reduce_window_opacity=20. Kitty: opacity 0.5, blur 30.
oh-my-posh print primary --config ~/.config/oh-my-posh/kolira.omp.json --shell fish
завершается успешно и рендерит prompt без CONFIG NOT FOUND после Apply.
Текущие ночные обои Minecraft существуют и успешно декодируются QImage.
Внешний вид всех уже открытых Qt-приложений не объявляется визуально проверенным;
они могут загрузить обновлённый стиль при следующем запуске.
