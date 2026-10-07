"""Regenerate small synthetic MIT-licensed fonts with fontTools (offline)."""
from pathlib import Path
from fontTools.fontBuilder import FontBuilder
from fontTools.pens.ttGlyphPen import TTGlyphPen
from fontTools.pens.t2CharStringPen import T2CharStringPen
from fontTools.feaLib.builder import addOpenTypeFeaturesFromString

root = Path(__file__).resolve().parent.parent / 'test' / 'fixtures'
order = ['.notdef', 'A', 'V', 'f', 'i', 'fi', 'zhong', 'wen', 'zero', 'one']
cmap = {ord(c): name for c, name in zip('AVfi中文01', ['A', 'V', 'f', 'i', 'zhong', 'wen', 'zero', 'one'])}
for is_ttf, suffix in [(True, 'ttf'), (False, 'otf')]:
    builder = FontBuilder(1000, isTTF=is_ttf)
    builder.setupGlyphOrder(order)
    builder.setupCharacterMap(cmap)
    outlines = {}
    for i, name in enumerate(order):
        pen = TTGlyphPen(None) if is_ttf else T2CharStringPen(500 + i * 11, None)
        pen.moveTo((30 + i, 0))
        pen.lineTo((250 + i, 700))
        pen.lineTo((470 + i, 0))
        pen.closePath()
        outlines[name] = pen.glyph() if is_ttf else pen.getCharString()
    if is_ttf:
        builder.setupGlyf(outlines)
    else:
        builder.setupCFF('StaluxSynthetic-Regular', {'FullName': 'Stalux Synthetic', 'FamilyName': 'Stalux Synthetic', 'Weight': 'Regular'}, outlines, {})
    builder.setupHorizontalMetrics({name: (500 + i * 11, 30 + i) for i, name in enumerate(order)})
    builder.setupHorizontalHeader(ascent=800, descent=-200)
    builder.setupVerticalMetrics({name: (1000 + i * 17, 100 + i) for i, name in enumerate(order)})
    builder.setupVerticalHeader(ascent=800, descent=-200)
    builder.setupNameTable({'familyName': 'Stalux Synthetic', 'styleName': 'Regular', 'uniqueFontIdentifier': 'StaluxSynthetic', 'fullName': 'Stalux Synthetic', 'psName': 'StaluxSynthetic-Regular', 'copyright': 'Copyright 2026 xingwangzhe. MIT License.'})
    builder.setupOS2(sTypoAscender=800, sTypoDescender=-200, usWinAscent=800, usWinDescent=200)
    builder.setupPost()
    builder.setupMaxp()
    addOpenTypeFeaturesFromString(builder.font, 'feature liga { sub f i by fi; } liga; feature kern { pos A V -80; } kern;')
    builder.font['head'].created = 3850070400
    builder.font['head'].modified = 3850070400
    builder.font.recalcTimestamp = False
    builder.font.save(root / ('SyntheticCJK.' + suffix))
