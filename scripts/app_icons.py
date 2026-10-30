"""Verify that native application artifacts retain the committed app icon."""

import ctypes
from pathlib import Path
import plistlib
import struct


def verify_macos_icon(app, source):
    """Check the bundle declaration and its copied icon, including relocated apps."""
    source = Path(source)
    icon = Path(app) / "Contents/Resources/YM_Link.icns"
    if not source.is_file() or not icon.is_file():
        raise ValueError("missing required application icon: YM_Link.icns")
    with (Path(app) / "Contents/Info.plist").open("rb") as stream:
        metadata = plistlib.load(stream)
    if metadata.get("CFBundleIconFile") != "YM_Link.icns":
        raise ValueError("CFBundleIconFile must reference YM_Link.icns")
    if icon.read_bytes() != source.read_bytes():
        raise ValueError("bundled YM_Link.icns differs from the committed application icon")


def ico_images(source):
    """Read the dimensions and encoded frames of an ICO without image libraries."""
    data = Path(source).read_bytes()
    if len(data) < 6:
        raise ValueError("invalid application ICO header")
    reserved, kind, count = struct.unpack_from("<HHH", data)
    if reserved != 0 or kind != 1 or not count or len(data) < 6 + 16 * count:
        raise ValueError("invalid application ICO directory")
    images = []
    for index in range(count):
        width, height, _, _, _, _, length, offset = struct.unpack_from("<BBBBHHII", data, 6 + 16 * index)
        if not length or offset < 6 + 16 * count or offset + length > len(data):
            raise ValueError("invalid application ICO frame")
        images.append((width, height, data[offset:offset + length]))
    return images


def verify_windows_icon(executable, source):
    """Inspect PE resources without executing the published Windows application."""
    from ctypes import wintypes

    expected = ico_images(source)
    kernel = ctypes.WinDLL("kernel32", use_last_error=True)
    kernel.LoadLibraryExW.argtypes = [wintypes.LPCWSTR, wintypes.HANDLE, wintypes.DWORD]
    kernel.LoadLibraryExW.restype = wintypes.HMODULE
    kernel.FreeLibrary.argtypes = [wintypes.HMODULE]
    kernel.FreeLibrary.restype = wintypes.BOOL
    kernel.FindResourceW.argtypes = [wintypes.HMODULE, ctypes.c_void_p, ctypes.c_void_p]
    kernel.FindResourceW.restype = wintypes.HRSRC
    kernel.SizeofResource.argtypes = [wintypes.HMODULE, wintypes.HRSRC]
    kernel.SizeofResource.restype = wintypes.DWORD
    kernel.LoadResource.argtypes = [wintypes.HMODULE, wintypes.HRSRC]
    kernel.LoadResource.restype = wintypes.HGLOBAL
    kernel.LockResource.argtypes = [wintypes.HGLOBAL]
    kernel.LockResource.restype = ctypes.c_void_p
    callback_type = ctypes.WINFUNCTYPE(wintypes.BOOL, wintypes.HMODULE, ctypes.c_void_p, ctypes.c_void_p, ctypes.c_ssize_t)
    kernel.EnumResourceNamesW.argtypes = [wintypes.HMODULE, ctypes.c_void_p, callback_type, ctypes.c_ssize_t]
    kernel.EnumResourceNamesW.restype = wintypes.BOOL

    # Data/image-resource flags prevent loading the app or any dependencies as code.
    module = kernel.LoadLibraryExW(str(Path(executable).resolve()), None, 0x00000002 | 0x00000020)
    if not module:
        raise ValueError("cannot inspect the published executable's application icon")
    try:
        def resource(name, kind):
            pointer = ctypes.c_void_p(name) if isinstance(name, int) else ctypes.cast(ctypes.c_wchar_p(name), ctypes.c_void_p)
            handle = kernel.FindResourceW(module, pointer, ctypes.c_void_p(kind))
            if not handle:
                raise ValueError("published executable is missing an application icon resource")
            size = kernel.SizeofResource(module, handle)
            address = kernel.LockResource(kernel.LoadResource(module, handle))
            if not size or not address:
                raise ValueError("published executable has an unreadable application icon resource")
            return ctypes.string_at(address, size)

        groups = []

        @callback_type
        def collect(_module, _kind, name, _parameter):
            name = name or 0
            groups.append(name if name <= 0xFFFF else ctypes.wstring_at(name))
            return True

        if not kernel.EnumResourceNamesW(module, ctypes.c_void_p(14), collect, 0):  # RT_GROUP_ICON
            raise ValueError("published executable has no application icon group")
        for name in groups:
            group = resource(name, 14)
            if len(group) < 6:
                continue
            reserved, kind, count = struct.unpack_from("<HHH", group)
            if reserved != 0 or kind != 1 or count != len(expected) or len(group) != 6 + 14 * count:
                continue
            actual = []
            for index in range(count):
                width, height, _, _, _, _, length, identifier = struct.unpack_from("<BBBBHHIH", group, 6 + 14 * index)
                frame = resource(identifier, 3)  # RT_ICON
                if len(frame) != length:
                    break
                actual.append((width, height, frame))
            if sorted(actual) == sorted(expected):
                return
        raise ValueError("published executable icon differs from the committed application icon")
    finally:
        kernel.FreeLibrary(module)
