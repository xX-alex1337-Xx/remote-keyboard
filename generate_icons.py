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


def draw_smooth_arc(draw, center, radius, start_deg, end_deg, stroke_width, fill_color):
    import math
    cx, cy = center
    r_outer = radius + stroke_width / 2.0
    draw.arc([cx - r_outer, cy - r_outer, cx + r_outer, cy + r_outer],
             start=start_deg, end=end_deg, fill=fill_color, width=int(stroke_width))

    # Rounded end caps matching stroke radius
    rad_start = math.radians(start_deg)
    x1 = cx + radius * math.cos(rad_start)
    y1 = cy + radius * math.sin(rad_start)
    cap_r = stroke_width / 2.0
    draw.ellipse([x1 - cap_r, y1 - cap_r, x1 + cap_r, y1 + cap_r], fill=fill_color)

    rad_end = math.radians(end_deg)
    x2 = cx + radius * math.cos(rad_end)
    y2 = cy + radius * math.sin(rad_end)
    draw.ellipse([x2 - cap_r, y2 - cap_r, x2 + cap_r, y2 + cap_r], fill=fill_color)

def get_highres_keyboard_glyph():
    font = ImageFont.truetype("C:/Windows/Fonts/SegoeIcons.ttf", 600)
    scratch = Image.new("RGBA", (800, 800), (0, 0, 0, 0))
    d = ImageDraw.Draw(scratch)
    d.text((50, 50), "\uE765", font=font, fill=(255, 255, 255, 255))
    return scratch.crop(scratch.getbbox())

KB_GLYPH_RAW = None

def render_fluent_remote_keyboard_icon(size, bg_color=None):
    global KB_GLYPH_RAW
    if KB_GLYPH_RAW is None:
        KB_GLYPH_RAW = get_highres_keyboard_glyph()

    # Master vector canvas at 1200x1200 for subpixel anti-aliasing
    master_size = 1200
    img = Image.new("RGBA", (master_size, master_size), (0, 0, 0, 0))

    kb_w = int(master_size * 0.82)
    kb_h = int(KB_GLYPH_RAW.height * (kb_w / KB_GLYPH_RAW.width))
    kb_scaled = KB_GLYPH_RAW.resize((kb_w, kb_h), Image.Resampling.LANCZOS)

    # Stroke thickness = EXACTLY same thickness as keyboard outer border (ratio = 37 / 600)
    stroke = int(kb_w * (37.0 / 600.0))

    # Keyboard positioned towards bottom
    kb_x = (master_size - kb_w) // 2
    kb_y = master_size - kb_h - int(master_size * 0.08)
    img.paste(kb_scaled, (kb_x, kb_y), kb_scaled)

    # Arc placed further away with clear airy gap:
    # Arc top at 8% of master_size (~96px), radius 44% (~528px)
    cx = master_size // 2
    r = int(master_size * 0.44)
    cy = int(master_size * 0.08) + r

    d_img = ImageDraw.Draw(img)
    # Shallow 50° North wave (245° to 295°) with rounded caps matching stroke radius
    draw_smooth_arc(d_img, (cx, cy), r, 245, 295, stroke, (255, 255, 255, 255))

    if bg_color:
        final_img = Image.new("RGBA", (size, size), bg_color)
    else:
        final_img = Image.new("RGBA", (size, size), (0, 0, 0, 0))

    downsampled = img.resize((size, size), Image.Resampling.LANCZOS)
    final_img.paste(downsampled, (0, 0), downsampled)
    return final_img

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
