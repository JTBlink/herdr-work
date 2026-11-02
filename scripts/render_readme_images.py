"""Render the redacted terminal snapshots used in README (requires Pillow)."""
import argparse
import json
from pathlib import Path
import re
import unicodedata

from PIL import Image, ImageDraw, ImageFont

ROOT = Path(__file__).resolve().parents[1]
ASSETS = ROOT / 'docs/verification/assets'


def render(font_path):
    frames = json.loads((ASSETS / 'readme-ui-020.json').read_text())
    icons = ImageFont.truetype(str(font_path), 22)
    mono = ImageFont.truetype('/System/Library/Fonts/Menlo.ttc', 22)
    symbols = ImageFont.truetype('/System/Library/Fonts/Apple Symbols.ttf', 22)
    cjk = ImageFont.truetype('/System/Library/Fonts/STHeiti Medium.ttc', 22)
    label = ImageFont.truetype('/System/Library/Fonts/STHeiti Medium.ttc', 21)
    cell_w, cell_h = 15, 29
    background, foreground, border = '#1d2130', '#e2e6ee', '#69768e'

    def panel(draw, key, x, y, width, row_count=22):
        frame = frames[key]
        text = frame['text']
        assert not re.search(r'/Users/|/home/|qihoo|JTStudio|@|(?:token|secret|password)\s*[:=]', text, re.I)
        lines = text.splitlines()
        rows = lines[:row_count - 4] + [''] + [' ' * ((width - 1) // 2) + '⋮'] + lines[-2:]
        draw.rectangle((x, y, x + (width + 2) * cell_w, y + len(rows) * cell_h + 20), outline=border, width=2)
        for row, line in enumerate(rows):
            column = 0
            for char in line:
                if unicodedata.category(char) in ('Mn', 'Me', 'Cf'):
                    continue
                advance = 2 if unicodedata.east_asian_width(char) in ('W', 'F') else 1
                if char.strip() and column + advance <= width:
                    font = (cjk if '\u3400' <= char <= '\u9fff' else
                            icons if unicodedata.category(char) == 'Co' else
                            symbols if char in '⋮⟳' else mono)
                    draw.text((x + (column + 1) * cell_w, y + row * cell_h + 8), char, font=font, fill=foreground)
                column += advance

    image = Image.new('RGB', (960, 825), background)
    draw = ImageDraw.Draw(image)
    draw.text((30, 22), 'HerdrWork 0.1.20 · 文件与 Git', font=label, fill=foreground)
    for key, x, title in [('explorer', 30, '文件'), ('git', 510, 'Git')]:
        draw.text((x, 65), title, font=label, fill=foreground)
        frame = frames[key]
        panel(draw, key, x, 105, frame['width'] - frame['border_columns'])
    draw.text((30, 790), '隔离演示仓库 · 真实终端文本渲染 · 中间空白省略', font=label, fill='#adb7ca')
    image.save(ASSETS / 'workbench.png', optimize=True)

    image = Image.new('RGB', (810, 655), background)
    draw = ImageDraw.Draw(image)
    draw.text((30, 22), 'HerdrWork 0.1.20 · 折叠与展开', font=label, fill=foreground)
    draw.text((30, 69), '内容 3 列', font=label, fill=foreground)
    panel(draw, 'collapsed', 45, 110, 3, 16)
    draw.text((230, 69), '展开后恢复原宽度', font=label, fill=foreground)
    panel(draw, 'explorer', 230, 110, frames['explorer']['width'] - 2, 16)
    draw.text((30, 605), '双边框总宽 5 列 · 按钮之间空一行 · 图标居中', font=label, fill='#adb7ca')
    image.save(ASSETS / 'sidebar-width-095.png', optimize=True)
    for name in ['workbench.png', 'sidebar-width-095.png']:
        with Image.open(ASSETS / name) as result:
            assert result.info == {}, 'Published PNG must not contain metadata'
        print(name)


if __name__ == '__main__':
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--font', type=Path, required=True, help='Path to a Nerd Font Mono TTF; macOS renderer')
    render(parser.parse_args().font)
