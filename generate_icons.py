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

def get_exact_24x24_original():
    data = base64.b64decode(AUTHENTIC_24X17_MASK_B64)
    orig_arr = np.frombuffer(data, dtype=np.uint8).reshape((17, 24))

    icon_24 = Image.new('RGBA', (24, 24), (0, 0, 0, 0))
    for y in range(17):
        for x in range(24):
            a = int(orig_arr[y, x])
            if a > 0:
                icon_24.putpixel((x, y + 3), (255, 255, 255, a))
    return icon_24

def render_original_keyboard_icon(size, bg_color=None):
    if size == 24:
        icon = get_exact_24x24_original()
        if bg_color:
            bg = Image.new('RGBA', (24, 24), bg_color)
            bg.paste(icon, (0, 0), icon)
            return bg
        return icon

    # High-resolution genuine Windows 11 Touch Keyboard glyph (\uE765) from SegoeIcons.ttf
    scale = 4
    canvas_size = size * scale
    font_size = int(canvas_size * 0.95)
    font = ImageFont.truetype("C:/Windows/Fonts/SegoeIcons.ttf", font_size)

    scratch_size = canvas_size * 2
    temp = Image.new("RGBA", (scratch_size, scratch_size), (0, 0, 0, 0))
    ImageDraw.Draw(temp).text((50, 50), "\uE765", font=font, fill=(255, 255, 255, 255))
    bbox = temp.getbbox()
    kb = temp.crop(bbox)

    target_w = int(canvas_size * 0.96)
    target_h = int(kb.height * (target_w / kb.width))
    if target_h > canvas_size * 0.96:
        target_h = int(canvas_size * 0.96)
        target_w = int(kb.width * (target_h / kb.height))

    kb_scaled = kb.resize((target_w, target_h), Image.Resampling.LANCZOS)

    final_img = Image.new("RGBA", (canvas_size, canvas_size), bg_color if bg_color else (0, 0, 0, 0))
    x = (canvas_size - target_w) // 2
    y = (canvas_size - target_h) // 2
    final_img.paste(kb_scaled, (x, y), kb_scaled)

    return final_img.resize((size, size), Image.Resampling.LANCZOS)

def main():
    os.makedirs("assets", exist_ok=True)
    os.makedirs("static", exist_ok=True)

    sizes = [16, 20, 24, 32, 48, 64, 128, 256]
    images = []
    for s in sizes:
        img = render_original_keyboard_icon(s, bg_color=None)
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

    ios_touch = render_original_keyboard_icon(180, bg_color=(0, 0, 0, 255))
    ios_touch.save("static/apple-touch-icon.png", format="PNG")
    print("Created static/apple-touch-icon.png")

    pwa_192 = render_original_keyboard_icon(192, bg_color=(0, 0, 0, 255))
    pwa_192.save("static/icon-192.png", format="PNG")

    pwa_512 = render_original_keyboard_icon(512, bg_color=(0, 0, 0, 255))
    pwa_512.save("static/icon-512.png", format="PNG")
    print("Created static/icon-192.png and static/icon-512.png")

if __name__ == "__main__":
    main()
