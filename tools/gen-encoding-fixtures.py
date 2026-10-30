#!/usr/bin/env python3
"""Regenerate test-data/encodings/fixtures/ from the UTF-8 corpus.

Each fixture is one corpus sample (test-data/encodings/corpus/<lang>.txt)
written in another encoding, named <encoding>-<lang>.txt. The hygg integration
test (packages/hygg/tests/integration_test/encoding_tests.rs) opens every
fixture through the real binary and checks the text matches the corpus.

Python's codecs are used on purpose: they are an encoder independent of
encoding_rs, which the decoder under test is built on.

Usage: python3 tools/gen-encoding-fixtures.py   (from anywhere; stdlib only)
"""

import pathlib

ROOT = pathlib.Path(__file__).resolve().parent.parent / "test-data" / "encodings"
CORPUS = ROOT / "corpus"
FIXTURES = ROOT / "fixtures"

# (file name prefix, python codec, prefix bytes, line ending)
UTF8_BOM = b"\xef\xbb\xbf"
FIXTURE_SPECS = [
    ("utf8-bom", "english", "utf-8", UTF8_BOM, "\n"),
    ("utf8-crlf", "german", "utf-8", b"", "\r\n"),
    # What Windows Notepad's "Unicode" option writes: UTF-16LE, BOM, CRLF.
    ("utf16le-bom", "spanish", "utf-16-le", b"\xff\xfe", "\r\n"),
    ("utf16be-bom", "greek", "utf-16-be", b"\xfe\xff", "\n"),
    ("utf16le-nobom", "portuguese", "utf-16-le", b"", "\n"),
    ("utf16le-nobom", "chinese_simplified", "utf-16-le", b"", "\n"),
    ("utf32le-bom", "korean", "utf-32-le", b"\xff\xfe\x00\x00", "\n"),
    ("utf32be-bom", "emoji", "utf-32-be", b"\x00\x00\xfe\xff", "\n"),
    ("utf7-bom", "english", "utf-7", None, "\n"),
    ("gb18030-bom", "chinese_simplified", "gb18030", None, "\n"),
    ("windows-1252", "french", "cp1252", b"", "\r\n"),
    ("iso-8859-1", "icelandic", "latin-1", b"", "\n"),
    ("iso-8859-2", "polish", "iso8859_2", b"", "\n"),
    ("windows-1250", "czech", "cp1250", b"", "\n"),
    ("windows-1254", "turkish", "cp1254", b"", "\n"),
    ("windows-1251", "ukrainian", "cp1251", b"", "\n"),
    ("koi8r", "russian", "koi8_r", b"", "\n"),
    ("windows-1256", "arabic", "cp1256", b"", "\n"),
    ("windows-1255", "hebrew", "cp1255", b"", "\n"),
    ("windows-874", "thai", "cp874", b"", "\n"),
    ("shift_jis", "japanese", "shift_jis", b"", "\r\n"),
    ("euc-jp", "japanese", "euc_jp", b"", "\n"),
    ("iso-2022-jp", "japanese", "iso2022_jp", b"", "\n"),
    ("gbk", "chinese_simplified", "gbk", b"", "\n"),
    ("big5", "chinese_traditional", "big5", b"", "\n"),
    ("euc-kr", "korean", "euc_kr", b"", "\n"),
]


def main():
    FIXTURES.mkdir(parents=True, exist_ok=True)
    for old in FIXTURES.glob("*.txt"):
        old.unlink()
    for prefix, lang, codec, bom, eol in FIXTURE_SPECS:
        text = (CORPUS / f"{lang}.txt").read_text(encoding="utf-8")
        text = text.replace("\n", eol)
        if bom is None:
            # The codec writes its own signature (U+FEFF encoded in it).
            data = ("﻿" + text).encode(codec)
        else:
            data = bom + text.encode(codec)
        (FIXTURES / f"{prefix}-{lang}.txt").write_bytes(data)
        print(f"{prefix}-{lang}.txt  {len(data)} bytes")


if __name__ == "__main__":
    main()
