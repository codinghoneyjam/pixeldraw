//! Common string constants (port of `src/ui/shared/strings.js`).

pub const APP_NAME: &str = "Pixeldraw";
pub const UNTITLED: &str = "제목 없음";
pub const UNTITLED_DOC: &str = "제목 없는 문서";
pub const LAYER_PREFIX: &str = "레이어 ";
pub const COPY_SUFFIX: &str = " 복사본";

pub const TOOL_TIPS: &[(&str, &str)] = &[
    ("pen", "펜 (B)"),
    ("eraser", "지우개 (E)"),
    ("line", "직선 (L)"),
    ("rect", "사각형 (R)"),
    ("ellipse", "타원 (U)"),
    ("fill", "채우기 (F)"),
    ("eyedropper", "스포이드 (I)"),
    ("hand", "손 (H)"),
    ("text", "텍스트 (T)"),
    ("gradient", "그라디언트 (G)"),
    ("shape", "도형 (M)"),
];

pub fn tool_tip(id: &str) -> &str {
    TOOL_TIPS
        .iter()
        .find(|(k, _)| *k == id)
        .map(|(_, v)| *v)
        .unwrap_or("도구")
}

pub fn layer_name(index: usize) -> String {
    format!("{}{}", LAYER_PREFIX, index + 1)
}

pub fn copy_name(name: &str) -> String {
    format!("{}{}", name, COPY_SUFFIX)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tool_tips() {
        assert_eq!(tool_tip("pen"), "펜 (B)");
        assert_eq!(tool_tip("eraser"), "지우개 (E)");
        assert_eq!(tool_tip("unknown"), "도구");
    }

    #[test]
    fn layer_names() {
        assert_eq!(layer_name(0), "레이어 1");
        assert_eq!(layer_name(4), "레이어 5");
    }

    #[test]
    fn copy_names() {
        assert_eq!(copy_name("레이어 1"), "레이어 1 복사본");
        assert_eq!(copy_name("Background"), "Background 복사본");
    }
}
