//! The tool table and the Toolbar constructor.
//!
//! Split out of `toolbar.rs`: the table is data (which tool, which key,
//! which help line) and the panel is behaviour, so they live apart. It is
//! also where the "every tool has a shortcut" tests point at.
// `Toolbar` is pure data; its behaviour lives in the parent `toolbar` module.


/// The tool grid: a fixed table of buttons plus the hovered help line.
pub struct Toolbar {
    pub(crate) tools: Vec<ToolButton>,
}

#[derive(Clone)]
pub(crate) struct ToolButton {
    pub(crate) id: &'static str,
    pub(crate) label: &'static str,
    /// Bare key, shown on the button. Every one of these is wired in
    /// `actions.rs` (`Shortcut::new(B, Modifiers::NONE, ToolPen)` and so on),
    /// so displaying it is stating a fact rather than a promise.
    pub(crate) shortcut: &'static str,
    /// One line shown on hover. The JS tree had these; the port dropped them,
    /// which is why hovering a tool said nothing and the option bar's generic
    /// branch was the only text on screen.
    pub(crate) help: &'static str,
}


impl Toolbar {
    pub fn new() -> Self {
        Self {
            // No pictographic icons. This font has no U+1F9FD (sponge), U+1FAA3
            // (bucket), U+1F4A7 (droplet), U+1D335 (double-struck T),
            tools: vec![
                ToolButton {
                    id: "pen",
                    label: "??,
                    shortcut: "B",
                    help: "?먯쑀濡?쾶 洹몃┰?덈떎. ?쒕옒洹명븯硫??댁뼱吏묐땲??",
                },
                ToolButton {
                    id: "eraser",
                    label: "吏?곌컻",
                    shortcut: "E",
                    help: "吏???먮━???щ챸?댁쭛?덈떎.",
                },
                ToolButton {
                    id: "fill",
                    label: "梨꾩슦湲?,
                    shortcut: "G",
                    help: "?꾨Ⅸ ?됱쑝濡??곗냽???곸뿭??梨꾩썎?덈떎.",
                },
                ToolButton {
                    id: "eyedropper",
                    label: "?ㅽ룷?대뱶",
                    shortcut: "I",
                    help: "?꾨Ⅸ 怨녹쓽 ?됱쓣 ?꾩옱 ?됱쑝濡?媛?몄샃?덈떎.",
                },
                ToolButton {
                    id: "line",
                    label: "吏곸꽑",
                    shortcut: "L",
                    help: "?꾨Ⅸ 怨녹뿉???볦? 怨녠퉴吏 ??以꾩쓣 湲뗭뒿?덈떎.",
                },
                ToolButton {
                    id: "rect",
                    label: "?ш컖??,
                    shortcut: "R",
                    help: "?꾨Ⅸ 怨녹뿉???볦? 怨녠퉴吏 吏곸궗媛곹삎??洹몃┰?덈떎.",
                },
                ToolButton {
                    id: "rrect",
                    label: "?κ렐?ш컖??,
                    shortcut: "U",
                    help: "紐⑥꽌由ш? ?κ렐 吏곸궗媛곹삎??洹몃┰?덈떎.",
                },
                ToolButton {
                    id: "ellipse",
                    label: "???,
                    shortcut: "O",
                    help: "?꾨Ⅸ 怨녹뿉???볦? 怨녠퉴吏 ??먯쓣 洹몃┰?덈떎.",
                },
                ToolButton {
                    id: "polygon",
                    label: "?ㅺ컖??,
                    shortcut: "P",
                    help: "?대┃?쇰줈 ?뺤젏???볤퀬 Enter濡??뺤젙?⑸땲??",
                },
                ToolButton {
                    id: "arc",
                    label: "??,
                    shortcut: "A",
                    help: "?꾨Ⅸ 怨녹뿉???볦? 怨녠퉴吏 ?몃? 洹몃┰?덈떎.",
                },
                ToolButton {
                    id: "chord",
                    label: "??,
                    shortcut: "C",
                    help: "?몄뿉 ?꾩쓣 ?뷀빐 遺梨꾧섦??洹몃┰?덈떎.",
                },
                ToolButton {
                    id: "radial_gradient",
                    label: "洹몃씪?붿뼵??,
                    shortcut: "V",
                    help: "?꾨Ⅸ 怨녹뿉???볦? 怨녠퉴吏 諛⑹궗??洹몃씪?붿뼵?몃? 洹몃┰?덈떎.",
                },
                ToolButton {
                    id: "text",
                    label: "?띿뒪??,
                    shortcut: "T",
                    help: "?뚮윭???낅젰????Esc濡??뺤젙?⑸땲??",
                },
                ToolButton {
                    id: "hand",
                    label: "??,
                    shortcut: "H",
                    help: "?붾㈃???뚯뼱??遊낅땲??",
                },
            ],
        }
    }
}

impl Default for Toolbar {
    fn default() -> Self {
        Self::new()
    }
}

