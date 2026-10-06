# <META - FILE SUMMARY - Nine-slice geometry primitives for scalable UI frames>
"""Nine-slice geometry primitives."""
from PIL import Image, ImageDraw

Color = tuple[int, int, int, int]

class NineSliceBuilder:
    # <META - ROLE : Store border, notch, and color configuration | L9-20>
    def __init__(
        self,
        *,
        border: int = 2,
        notch: int = 4,
        background: Color = (0, 0, 0, 0),
        foreground: Color = (255, 255, 255, 255),
    ) -> None:
        self.border: int = border
        self.notch: int = notch
        self.background: Color = background
        self.foreground: Color = foreground

    # <META - ROLE : Generate a nine-slice polygon outline for given dimensions | L23-29>
    def polygon(self, width: int, height: int) -> list[tuple[int, int]]:
        n = min(self.notch, width // 2, height // 2)
        x1, y1 = width - 1, height - 1
        return [(0, n), (n, n), (n, 0), (x1 - n, 0),
                (x1 - n, n), (x1, n), (x1, y1 - n),
                (x1 - n, y1 - n), (x1 - n, y1), (n, y1),
                (n, y1 - n), (0, y1 - n)]

    # <META - ROLE : Render a nine-slice frame image at the specified size | L32-37>
    def render(self, size: tuple[int, int]) -> Image.Image:
        image = Image.new("RGBA", size, (0, 0, 0, 0))
        draw = ImageDraw.Draw(image)
        points = self.polygon(*size)
        draw.polygon(points, fill=self.background, outline=self.foreground, width=self.border)
        return image
