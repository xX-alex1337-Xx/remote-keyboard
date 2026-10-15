import os
from PIL import Image, ImageDraw, ImageFont

def render_centered_icon(size, bg_color=None, padding_ratio=0.08):
    scale = 4
    canvas_size = size * scale
    
    # 1. High-resolution scratch canvas
    scratch_size = canvas_size * 2
    scratch = Image.new("RGBA", (scratch_size, scratch_size), (0, 0, 0, 0))
    draw = ImageDraw.Draw(scratch)
    white = (255, 255, 255, 255)
    
    font_size = int(canvas_size * 0.68)
    font = ImageFont.truetype("C:/Windows/Fonts/SegoeIcons.ttf", font_size)
    
    # Draw keyboard at scratch center
    kb_x = int(scratch_size * 0.25)
    kb_y = int(scratch_size * 0.35)
    draw.text((kb_x, kb_y), "\uE765", font=font, fill=white)
    
    # Measure keyboard bounds
    kb_img_temp = Image.new("RGBA", (scratch_size, scratch_size), (0, 0, 0, 0))
    ImageDraw.Draw(kb_img_temp).text((kb_x, kb_y), "\uE765", font=font, fill=white)
    bbox = kb_img_temp.getbbox()
    
    kb_w = bbox[2] - bbox[0]
    kb_h = bbox[3] - bbox[1]
    kb_corner_x = bbox[2]
    kb_corner_y = bbox[1]
    
    # Signal arcs radiating from top-right corner
    origin_x = kb_corner_x - int(kb_w * 0.10)
    origin_y = kb_corner_y + int(kb_h * 0.12)
    stroke = max(2 * scale, int(font_size * 0.058))
    
    r1 = int(kb_h * 0.28)
    r2 = int(kb_h * 0.52)
    
    draw.arc([origin_x - r1, origin_y - r1, origin_x + r1, origin_y + r1],
             start=265, end=365, fill=white, width=stroke)
    draw.arc([origin_x - r2, origin_y - r2, origin_x + r2, origin_y + r2],
             start=265, end=365, fill=white, width=stroke)
             
    # Now get the combined bounding box of keyboard + arcs
    combo_bbox = scratch.getbbox()
    combo_w = combo_bbox[2] - combo_bbox[0]
    combo_h = combo_bbox[3] - combo_bbox[1]
    
    # Crop the exact glyph
    cropped = scratch.crop(combo_bbox)
    
    # Scale cropped image to fit within target canvas leaving padding
    avail_w = canvas_size * (1.0 - 2 * padding_ratio)
    avail_h = canvas_size * (1.0 - 2 * padding_ratio)
    
    scale_factor = min(avail_w / combo_w, avail_h / combo_h)
    new_w = max(1, int(combo_w * scale_factor))
    new_h = max(1, int(combo_h * scale_factor))
    
    scaled_glyph = cropped.resize((new_w, new_h), Image.Resampling.LANCZOS)
    
    # Final canvas with background
    if bg_color:
        final_canvas = Image.new("RGBA", (canvas_size, canvas_size), bg_color)
    else:
        final_canvas = Image.new("RGBA", (canvas_size, canvas_size), (0, 0, 0, 0))
        
    pos_x = (canvas_size - new_w) // 2
    pos_y = (canvas_size - new_h) // 2
    final_canvas.paste(scaled_glyph, (pos_x, pos_y), scaled_glyph)
    
    # Downsample to target size
    return final_canvas.resize((size, size), Image.Resampling.LANCZOS)

def main():
    os.makedirs("assets", exist_ok=True)
    os.makedirs("static", exist_ok=True)
    
    sizes = [16, 20, 24, 32, 48, 64, 128, 256]
    images = []
    for s in sizes:
        img = render_centered_icon(s, bg_color=None, padding_ratio=0.06)
        images.append(img)
        if s == 32:
            img.save("static/favicon.png", format="PNG")
        if s == 48:
            img.save("assets/icon_48.png", format="PNG")
        if s == 256:
            img.save("assets/icon_256.png", format="PNG")
            
    images[0].save(
        "assets/icon.ico",
        format="ICO",
        sizes=[(s, s) for s in sizes],
        append_images=images[1:]
    )
    print("Created assets/icon.ico")
    
    fav_images = [img for img, s in zip(images, sizes) if s in [16, 32, 48]]
    fav_images[0].save(
        "static/favicon.ico",
        format="ICO",
        sizes=[(img.width, img.height) for img in fav_images],
        append_images=fav_images[1:]
    )
    print("Created static/favicon.ico")
    
    # Apple Touch & PWA (pure OLED black background)
    ios_touch = render_centered_icon(180, bg_color=(0, 0, 0, 255), padding_ratio=0.18)
    ios_touch.save("static/apple-touch-icon.png", format="PNG")
    print("Created static/apple-touch-icon.png")

    pwa_192 = render_centered_icon(192, bg_color=(0, 0, 0, 255), padding_ratio=0.18)
    pwa_192.save("static/icon-192.png", format="PNG")
    
    pwa_512 = render_centered_icon(512, bg_color=(0, 0, 0, 255), padding_ratio=0.18)
    pwa_512.save("static/icon-512.png", format="PNG")
    print("Created static/icon-192.png and static/icon-512.png")

if __name__ == "__main__":
    main()
