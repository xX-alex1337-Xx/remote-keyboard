import os
from PIL import Image, ImageFont, ImageDraw
import numpy as np

import base64

# Authentic Windows 11 Taskbar Touch Keyboard pixel alpha mask (24x17)
AUTHENTIC_24X17_MASK_B64 = (
    "ALz1////////////////////////9bwAvP/HsbGxsbGxsbGxsbGxsbGxsbGxx/+89ccAAAAAAAAAAAAAAAAAAAAAAAAAAMf1"
    "/7EAAFc9AAAfbgAAAFc9AAAfbgAAALH//7EAV//1AADR/6MAV//1AADR/6MAALH//7EAPfXjAAC8/4IAPfXjAAC8/4IA"
    "ALH//7EAAAAAAAAAAAAAAAAAAAAAAAAAALH//7EAAAAAAAAAAAAAAAAAAAAAAAAAALH//7EAAAAAH9G8AACT2lcAH9G8AAAA"
    "ALH//7EAAAAAbv//AADa/7EAbv//AAAAALH//7EAAAAAAKOCAABXsR8AAKOCAAAAALH//7EAAAAAAAAAAAAAAAAAAAAAAAAA"
    "ALH//7EAAKP//////////////////6MAALH//7EAAFexsbGxsbGxsbGxsbGxsVcAALH/4+w9AAAAAAAAAAAAAAAAAAAA"
    "AAAAPezjV/X///////////////////////////VXAD2jsbGxsbGxsbGxsbGxsbGxsbGxoz0A"
)


def get_fluent_arc_raw():
    # Extract authentic Fluent WiFi arc from SegoeIcons.ttf glyph \uEC3E
    font_wifi = ImageFont.truetype("C:/Windows/Fonts/SegoeIcons.ttf", 300)
    temp_wifi = Image.new("RGBA", (400, 400), (0, 0, 0, 0))
    ImageDraw.Draw(temp_wifi).text((50, 50), "\uEC3E", font=font_wifi, fill=(255, 255, 255, 255))
    wifi_full = temp_wifi.crop(temp_wifi.getbbox())
    arc = wifi_full.crop((0, 0, wifi_full.width, int(wifi_full.height * 0.32)))
    return arc.crop(arc.getbbox())

def get_exact_24x24_with_arc():
    data = base64.b64decode(AUTHENTIC_24X17_MASK_B64)
    orig_arr = np.frombuffer(data, dtype=np.uint8).reshape((17, 24))

    icon_24 = Image.new('RGBA', (24, 24), (0, 0, 0, 0))
    # Keyboard placed at y=4, keeping 100% of authentic pixels untouched
    for y in range(17):
        for x in range(24):
            a = int(orig_arr[y, x])
            if a > 0:
                icon_24.putpixel((x, y + 4), (255, 255, 255, a))

    # Fluent arc: width 12, height 3, placed at y=0, x=6 (1px clean black gap at y=3)
    arc_raw = get_fluent_arc_raw()
    arc_scaled = arc_raw.resize((12, 3), Image.Resampling.LANCZOS)
    icon_24.paste(arc_scaled, (6, 0), arc_scaled)

    return icon_24

def render_fluent_remote_keyboard_icon(size, bg_color=None):
    if size == 24:
        icon = get_exact_24x24_with_arc()
        if bg_color:
            bg = Image.new('RGBA', (24, 24), bg_color)
            bg.paste(icon, (0, 0), icon)
            return bg
        return icon

    # Vector rendering from SegoeIcons.ttf for crisp multi-resolution display
    scale = 4
    canvas_size = size * scale

    font_kb = ImageFont.truetype("C:/Windows/Fonts/SegoeIcons.ttf", int(canvas_size * 0.82))
    temp_kb = Image.new("RGBA", (canvas_size * 2, canvas_size * 2), (0, 0, 0, 0))
    ImageDraw.Draw(temp_kb).text((50, 50), "\uE765", font=font_kb, fill=(255, 255, 255, 255))
    kb_raw = temp_kb.crop(temp_kb.getbbox())

    arc_raw = get_fluent_arc_raw()

    target_kb_w = int(canvas_size * 0.94)
    target_kb_h = int(kb_raw.height * (target_kb_w / kb_raw.width))
    kb_scaled = kb_raw.resize((target_kb_w, target_kb_h), Image.Resampling.LANCZOS)

    # Arc width = 50% of keyboard width
    target_arc_w = int(target_kb_w * 0.50)
    target_arc_h = int(arc_raw.height * (target_arc_w / arc_raw.width))
    arc_scaled = arc_raw.resize((target_arc_w, target_arc_h), Image.Resampling.LANCZOS)

    gap = max(scale, int(canvas_size * 0.045))
    total_h = target_arc_h + gap + target_kb_h
    start_y = (canvas_size - total_h) // 2

    final_img = Image.new("RGBA", (canvas_size, canvas_size), bg_color if bg_color else (0, 0, 0, 0))

    # Paste arc centered
    arc_x = (canvas_size - target_arc_w) // 2
    final_img.paste(arc_scaled, (arc_x, start_y), arc_scaled)

    # Paste keyboard centered
    kb_x = (canvas_size - target_kb_w) // 2
    final_img.paste(kb_scaled, (kb_x, start_y + target_arc_h + gap), kb_scaled)

    return final_img.resize((size, size), Image.Resampling.LANCZOS)

def main():
    os.makedirs("assets", exist_ok=True)
    os.makedirs("static", exist_ok=True)

    sizes = [16, 20, 24, 32, 48, 64, 128, 256]
    images = []
    for s in sizes:
        img = render_fluent_remote_keyboard_icon(s, bg_color=None)
        images.append(img)
        if s == 24:
            img.save("assets/icon_24.png", format="PNG")
        if s == 32:
            img.save("static/favicon.png", format="PNG")
        if s == 48:
            img.save("assets/icon_48.png", format="PNG")
        if s == 256:
            img.save("assets/icon_256.png", format="PNG")

    # Save ICO with largest image first to ensure Pillow preserves all multi-resolution frames
    # including 16, 20, 24, 32, 48, 64, 128, 256
    img_by_size = {s: img for s, img in zip(sizes, images)}
    all_sizes = sorted(sizes)
    largest_img = img_by_size[all_sizes[-1]]
    other_imgs = [img_by_size[s] for s in all_sizes[:-1]]

    largest_img.save(
        "assets/icon.ico",
        format="ICO",
        sizes=[(s, s) for s in all_sizes],
        append_images=other_imgs
    )
    print("Created assets/icon.ico with all multi-resolution frames (16 to 256)")

    fav_sizes = [16, 32, 48]
    fav_largest = img_by_size[48]
    fav_others = [img_by_size[16], img_by_size[32]]
    fav_largest.save(
        "static/favicon.ico",
        format="ICO",
        sizes=[(s, s) for s in fav_sizes],
        append_images=fav_others
    )
    print("Created static/favicon.ico")

    ios_touch = render_fluent_remote_keyboard_icon(180, bg_color=(0, 0, 0, 255))
    ios_touch.save("static/apple-touch-icon.png", format="PNG")
    print("Created static/apple-touch-icon.png")

    pwa_192 = render_fluent_remote_keyboard_icon(192, bg_color=(0, 0, 0, 255))
    pwa_192.save("static/icon-192.png", format="PNG")

    pwa_512 = render_fluent_remote_keyboard_icon(512, bg_color=(0, 0, 0, 255))
    pwa_512.save("static/icon-512.png", format="PNG")
    print("Created static/icon-192.png and static/icon-512.png")

if __name__ == "__main__":
    main()
