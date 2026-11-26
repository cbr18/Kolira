# Согласование радиусов скруглений и исключение Telegram/Kotatogram из Better Blur DX

Дата: 2026-11-25. Статус: **DONE 2026-11-26**. Реализовано в 9 комплектных темах;
dracula проверен вживую на Plasma (углы блюра скруглились). Ключевое исправление:
сам блюр скругляет Better Blur DX `CornerRadius`, а не `RefractionCornerRadius`
(тот отвечает только за «линзу» по краям) — `CornerRadius=R` добавлен во все темы.

## Задача

В каждой теме kolira согласовать радиус скругления во всех местах, где он задан,
чтобы использовалось одно значение R на тему:

- KDE: Klassy `WindowCornerRadius` (классические темы) и/или KWin `Round-Corners`
  `Size`/`InactiveCornerRadius` (Aurorae-темы);
- Better Blur DX: `RefractionCornerRadius` (и `CornerRadius`, если ключ есть);
- wofi: все `border-radius` (внешнее окно, `#input`, `#entry`, `#img`).

> **Важно (исправлено):** скругление самого блюра в Better Blur DX задаёт
> ключ `CornerRadius` (Double, «Adjustable corner radius», по умолчанию 0 =
> квадрат). `RefractionCornerRadius` управляет только «линзой»/преломлением по
> краям и блюр НЕ обрезает. Первая версия правок выставила только
> `RefractionCornerRadius`, из-за чего у Klassy-тем (dracula/glass/…) блюр
> оставался квадратным и затекал в углы. Теперь `CornerRadius=R` выставлен во
> всех темах.

Дополнительно: Better Blur DX должен **не** размывать Telegram и Kotatogram —
добавить их классы в список исключений `WindowClasses`.

## Решения (согласованы с пользователем)

1. **Источник радиуса — существующий радиус темы.** Одно значение R на тему,
   не единое для всех тем.
2. **wofi — всё = R.** Внешний контейнер, поля и элементы получают один радиус
   (внешняя рамка становится менее круглой, но значение единое).
3. **Исключение Telegram/Kotatogram — оба имени.** В `WindowClasses` добавлены
   `org.telegram.desktop` (Wayland app_id) и `TelegramDesktop` (X11
   `StartupWMClass`), а также `kotatogramdesktop` и `KotatogramDesktop`.

### Выбранные R по темам

| Тема | R | Декорация | Было → стало (ключевое) |
|---|---|---|---|
| glass | 10 | Klassy | BBDX Refraction 15→10; wofi 16/10/9/8→10 |
| dracula | 10 | Klassy | BBDX Refraction 15→10; wofi →10 |
| everforest | 10 | Klassy | BBDX Refraction 15→10; wofi →10 |
| material-you | 14 | Klassy | BBDX Refraction 15→14; wofi →14 |
| sakura-dream | 12 | Klassy | BBDX Refraction 15→12; wofi →12 |
| minecraft-overworld-dark | 0 | Klassy | BBDX Refraction 15→0; wofi уже 0 |
| frutiger-aero | 10 | Aurorae SVG | BBDX Refraction 15→10; wofi →10; SVG не трогаем |
| frutiger-aero-light | 10 | Aurorae SVG + Round-Corners | уже 10; wofi →10 |
| Paper | 0 | Breeze | wofi уже 0; BBDX `RefractionCornerRadius` добавлен =0 |

Примечание по `frutiger-aero`: у темы не было единого явного радиуса (BBDX 15,
wofi 16/10). Выбрано 10, чтобы совпасть с парной `frutiger-aero-light` и
внутренними элементами wofi. Геометрия углов в Aurorae SVG (`Se7enAero`) не
меняется — это baked-радиус в SVG, а не конфиг-ключ.

## Что не входит в область

- GTK-приложения (свои `border-radius`), rofi/fuzzel/walker — у них нет
  радиусов (только цвета), лаунчеры не трогаем.
- `klassy/windecopresetsrc` — библиотека пресетов, активный конфиг в `klassyrc`.
- Пользовательская тема `my-original-look` (только в live-каталоге, не в kit).

## Проверка

- `grep` по всем темам: `WindowCornerRadius`, `RefractionCornerRadius`,
  `CornerRadius`, `border-radius`, `WindowClasses` — значения совпадают с R,
  Telegram/Kotatogram присутствуют в исключениях.
- Rust/Python/shell-тесты не затронуты (правки только в данных тем), но
  запускаются по общему правилу перед коммитом; добавлен тест на наличие
  `CornerRadius` во всех темах с forced blur.
- Живая проверка на Plasma: dracula применена, KWin перечитал конфиг, углы блюра
  скруглились (подтверждено пользователем 2026-11-26). Остальные темы используют
  тот же механизм (`CornerRadius=R`).