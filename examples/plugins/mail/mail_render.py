#!/usr/bin/env python3
# mail_render.py — extract HTML body and images from RFC822 source.
#
# Reads the full RFC822 message source on stdin and emits one of:
#   --extract-html     prints the text/html alternative (or "" if none)
#   --extract-text     prints the text/plain alternative (or "" if none)
#   --save-images DIR  saves embedded MIME image parts AND remote
#                      <img src="https://..."> references from the HTML to
#                      DIR. Prints JSON list of {path, mime, cid, filename,
#                      disposition, source} per image. "source" is "embedded"
#                      or the remote URL.
#
# Used by examples/plugins/mail/inbox.lua to power the View body / View images
# chain actions. Mail.app's m.source() gives us the raw RFC822; everything
# downstream (pandoc, w3m, chafa) wants either HTML or decoded image files.
#
# Python stdlib only (email + base64 + urllib + html.parser) — works with
# /usr/bin/python3 out of the box on macOS without Homebrew Python.

import sys
import os
import json
import email
import urllib.request
import urllib.parse
from email import policy
from html.parser import HTMLParser


def _decode(part):
    """Best-effort decode of a MIME part to text. Honors charset, falls back
    to utf-8 with replacement so we never raise on weird encodings."""
    try:
        return part.get_content()
    except Exception:
        payload = part.get_payload(decode=True)
        if not payload:
            return ""
        charset = part.get_content_charset() or "utf-8"
        try:
            return payload.decode(charset, errors="replace")
        except LookupError:
            return payload.decode("utf-8", errors="replace")


def extract_alternative(msg, content_type):
    """Walk multipart structure and return the first part matching
    content_type as text. Prefers non-attachment dispositions."""
    candidates = []
    for part in msg.walk():
        if part.is_multipart():
            continue
        if part.get_content_type() != content_type:
            continue
        # Skip explicit attachments — we want inline body content.
        disp = (part.get_content_disposition() or "").lower()
        if disp == "attachment":
            continue
        candidates.append(part)
    if not candidates:
        return ""
    # Multiple HTML parts are unusual but possible; concatenate so the
    # downstream renderer sees the full content.
    return "\n".join(_decode(p) for p in candidates)


class _ImgSrcCollector(HTMLParser):
    """Collect <img src="..."> attribute values from HTML."""
    def __init__(self):
        super().__init__()
        self.srcs = []

    def handle_starttag(self, tag, attrs):
        if tag.lower() != "img":
            return
        for key, value in attrs:
            if key.lower() == "src" and value:
                self.srcs.append(value)


def _ext_for_mime(ct):
    ext = ct.split("/", 1)[1].split(";", 1)[0].strip() or "bin"
    return {"jpeg": "jpg", "svg+xml": "svg"}.get(ext, ext)


# Sanity limits: skip tracking-pixel-style content and cap network spend.
MIN_BYTES = 1024            # < 1 KB is almost always a tracking pixel
MAX_BYTES_PER_IMAGE = 4 * 1024 * 1024
MAX_REMOTE_IMAGES = 25
NET_TIMEOUT_SECS = 8


def _fetch_remote(url, dest_path):
    """Download url to dest_path. Returns (mime, size) on success, None on
    any failure. Enforces size + timeout caps so a hostile email can't make
    us hang or fill the disk."""
    try:
        req = urllib.request.Request(url, headers={
            "User-Agent": "larkline-mail-render/0.1",
            "Accept": "image/*,*/*;q=0.8",
        })
        with urllib.request.urlopen(req, timeout=NET_TIMEOUT_SECS) as resp:
            ct = (resp.headers.get_content_type() or "").lower()
            if not ct.startswith("image/"):
                return None
            data = resp.read(MAX_BYTES_PER_IMAGE + 1)
            if len(data) > MAX_BYTES_PER_IMAGE or len(data) < MIN_BYTES:
                return None
            with open(dest_path, "wb") as f:
                f.write(data)
            return ct, len(data)
    except Exception:
        return None


def save_images(msg, outdir):
    """Save embedded MIME image parts AND remote <img> srcs to outdir.

    Embedded parts come first (zero-network, cheap). Then we parse the HTML
    body for remote <img src="https://...">, dedupe, and download up to
    MAX_REMOTE_IMAGES of them. Tracking-pixel-sized payloads are skipped so
    the rendered output isn't dominated by 1x1 transparent GIFs."""
    os.makedirs(outdir, exist_ok=True)
    saved = []
    idx = 0

    # 1) Embedded MIME image parts.
    for part in msg.walk():
        if part.is_multipart():
            continue
        ct = part.get_content_type()
        if not ct.startswith("image/"):
            continue
        payload = part.get_payload(decode=True)
        if not payload:
            continue
        if len(payload) < MIN_BYTES:
            continue
        ext = _ext_for_mime(ct)
        filename = f"img{idx:03d}.{ext}"
        path = os.path.join(outdir, filename)
        with open(path, "wb") as f:
            f.write(payload)
        cid = (part.get("Content-ID") or "").strip("<>").strip()
        disp = (part.get_content_disposition() or "").lower()
        original = part.get_filename() or ""
        saved.append({
            "path": path,
            "mime": ct,
            "cid": cid,
            "filename": original or filename,
            "disposition": disp,
            "size": len(payload),
            "source": "embedded",
        })
        idx += 1

    # 2) Remote <img src="..."> references from the HTML alternative.
    html = extract_alternative(msg, "text/html")
    if html:
        parser = _ImgSrcCollector()
        try:
            parser.feed(html)
        except Exception:
            pass
        seen = set()
        remote_count = 0
        for src in parser.srcs:
            if remote_count >= MAX_REMOTE_IMAGES:
                break
            scheme = urllib.parse.urlparse(src).scheme.lower()
            if scheme not in ("http", "https"):
                continue
            if src in seen:
                continue
            seen.add(src)
            tmp_path = os.path.join(outdir, f"img{idx:03d}.bin")
            result = _fetch_remote(src, tmp_path)
            if result is None:
                continue
            ct, size = result
            ext = _ext_for_mime(ct)
            final_name = f"img{idx:03d}.{ext}"
            final_path = os.path.join(outdir, final_name)
            if final_path != tmp_path:
                os.rename(tmp_path, final_path)
            saved.append({
                "path": final_path,
                "mime": ct,
                "cid": "",
                "filename": final_name,
                "disposition": "remote",
                "size": size,
                "source": src,
            })
            idx += 1
            remote_count += 1

    return saved


def main():
    if len(sys.argv) < 2:
        sys.stderr.write("usage: mail_render.py {--extract-html|--extract-text|--save-images DIR}\n")
        sys.exit(2)
    src = sys.stdin.buffer.read()
    msg = email.message_from_bytes(src, policy=policy.default)
    cmd = sys.argv[1]
    if cmd == "--extract-html":
        sys.stdout.write(extract_alternative(msg, "text/html"))
    elif cmd == "--extract-text":
        sys.stdout.write(extract_alternative(msg, "text/plain"))
    elif cmd == "--save-images":
        if len(sys.argv) < 3:
            sys.stderr.write("--save-images requires a directory\n")
            sys.exit(2)
        outdir = sys.argv[2]
        sys.stdout.write(json.dumps(save_images(msg, outdir)))
    else:
        sys.stderr.write(f"unknown command: {cmd}\n")
        sys.exit(2)


if __name__ == "__main__":
    main()
