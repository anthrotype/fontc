"""Pytest configuration and fixtures."""

import tempfile
from pathlib import Path
import pytest


@pytest.fixture
def temp_dir():
    """Create a temporary directory for test outputs."""
    with tempfile.TemporaryDirectory() as tmpdir:
        yield Path(tmpdir)


@pytest.fixture
def minimal_ufo(temp_dir):
    """Create a minimal UFO font for testing."""
    ufo_dir = temp_dir / "test.ufo"
    ufo_dir.mkdir()

    # Create metainfo.plist
    metainfo = ufo_dir / "metainfo.plist"
    metainfo.write_text("""<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
    <key>creator</key>
    <string>test</string>
    <key>formatVersion</key>
    <integer>3</integer>
</dict>
</plist>
""")

    # Create fontinfo.plist
    fontinfo = ufo_dir / "fontinfo.plist"
    fontinfo.write_text("""<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
    <key>familyName</key>
    <string>Test</string>
    <key>styleName</key>
    <string>Regular</string>
    <key>unitsPerEm</key>
    <integer>1000</integer>
    <key>ascender</key>
    <integer>800</integer>
    <key>descender</key>
    <integer>-200</integer>
</dict>
</plist>
""")

    # Create glyphs directory with a simple glyph
    glyphs_dir = ufo_dir / "glyphs"
    glyphs_dir.mkdir()

    contents_plist = glyphs_dir / "contents.plist"
    contents_plist.write_text("""<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
    <key>.notdef</key>
    <string>_notdef.glif</string>
    <key>space</key>
    <string>space.glif</string>
</dict>
</plist>
""")

    notdef_glif = glyphs_dir / "_notdef.glif"
    notdef_glif.write_text("""<?xml version="1.0" encoding="UTF-8"?>
<glyph name=".notdef" format="2">
  <advance width="500"/>
  <outline>
    <contour>
      <point x="100" y="0" type="line"/>
      <point x="400" y="0" type="line"/>
      <point x="400" y="700" type="line"/>
      <point x="100" y="700" type="line"/>
    </contour>
  </outline>
</glyph>
""")

    space_glif = glyphs_dir / "space.glif"
    space_glif.write_text("""<?xml version="1.0" encoding="UTF-8"?>
<glyph name="space" format="2">
  <advance width="250"/>
  <unicode hex="0020"/>
</glyph>
""")

    return ufo_dir
