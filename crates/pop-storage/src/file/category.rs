use crate::app::Category;
const CATEGORY_COLUMNS: [(Category, &str); 14] = [
    (Category::Popular, "popular"),
    (Category::Productivity, "productivity"),
    (Category::Utilities, "utilities"),
    (Category::Finance, "finance"),
    (Category::Media, "media"),
    (Category::Games, "games"),
    (Category::DeveloperTools, "developer-tools"),
    (Category::Education, "education"),
    (Category::Business, "business"),
    (Category::Personal, "personal"),
    (Category::Health, "health"),
    (Category::Creative, "creative"),
    (Category::Music, "music"),
    (Category::Other, "other"),
];
#[cfg(not(target_arch = "wasm32"))]
pub(crate) fn category_column(category: Category) -> &'static str {
    CATEGORY_COLUMNS.iter().find(|(c, _)| *c == category).map(|(_, name)| *name).expect("every category has a column")
}
/// The category a stored spelling names. A spelling this build does not know can only come
/// from a later build that added a category, so it reads as `Other` and the file still lists
/// and opens. Authoring parses strictly (`Category::parse`).
fn column_category(name: &str) -> Category {
    CATEGORY_COLUMNS.iter().find(|(_, column)| *column == name).map_or(Category::Other, |(category, _)| *category)
}
/// The categories the two columns hold: one or two different ones, as `stored_metadata`
/// requires. Spellings that read as one category are one.
pub(crate) fn column_categories(primary: &str, secondary: Option<&str>) -> Vec<Category> {
    let mut categories = vec![column_category(primary)];
    if let Some(secondary) = secondary.map(column_category)
        && !categories.contains(&secondary)
    {
        categories.push(secondary);
    }
    categories
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn every_category_has_one_stored_spelling() {
        for &category in Category::ALL {
            assert_eq!(column_category(category_column(category)), category);
        }
        assert_eq!(CATEGORY_COLUMNS.len(), Category::ALL.len());
    }
}
