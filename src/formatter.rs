use crate::state::ArticleSnapshot;

pub const DISCORD_CONTENT_LIMIT: usize = 2000;

const TITLE_LIMIT: usize = 200;
const LINK_LIMIT: usize = 500;
const ELLIPSIS: &str = "...";

pub fn format_threshold_message(
    article: &ArticleSnapshot,
    threshold: u64,
    bookmark_count: u64,
) -> String {
    let title = clean_and_truncate(&article.title, TITLE_LIMIT, "No Title");
    let link = if article.url.trim().is_empty() {
        "No link".to_string()
    } else {
        truncate(article.url.trim(), LINK_LIMIT)
    };
    let label = match threshold {
        1 => "New Hatena Bookmark Item",
        5 => "Hatena Bookmark Rising Item",
        20 => "Hatena Bookmark Hot Item",
        _ => "Hatena Bookmark Threshold Item",
    };

    let content = format!(
        "**{label}**\n\n**Threshold:** {threshold} bookmarks\n**Bookmarks:** {bookmark_count}\n**Title:** {title}\n**Link:** {link}",
    );

    truncate_preserving_threshold_link(&content, threshold, bookmark_count, &link)
}

fn clean_and_truncate(value: &str, limit: usize, fallback: &str) -> String {
    let cleaned = sanitize_mentions(&normalize_whitespace(&strip_html_tags(value)));

    if cleaned.is_empty() {
        fallback.to_string()
    } else {
        truncate(&cleaned, limit)
    }
}

fn strip_html_tags(value: &str) -> String {
    let mut output = String::new();
    let mut in_tag = false;

    for ch in value.chars() {
        match ch {
            '<' => in_tag = true,
            '>' => in_tag = false,
            _ if !in_tag => output.push(ch),
            _ => {}
        }
    }

    output
}

fn normalize_whitespace(value: &str) -> String {
    value.split_whitespace().collect::<Vec<_>>().join(" ")
}

fn sanitize_mentions(value: &str) -> String {
    value
        .replace("@everyone", "@\u{200b}everyone")
        .replace("@here", "@\u{200b}here")
        .replace("<@&", "<@\u{200b}&")
        .replace("<@", "<@\u{200b}")
}

fn truncate(value: &str, limit: usize) -> String {
    if value.chars().count() <= limit {
        return value.to_string();
    }

    let keep = limit.saturating_sub(ELLIPSIS.chars().count());
    let mut output = value.chars().take(keep).collect::<String>();
    output.push_str(ELLIPSIS);
    output
}

fn truncate_preserving_threshold_link(
    content: &str,
    threshold: u64,
    bookmark_count: u64,
    link: &str,
) -> String {
    if content.chars().count() <= DISCORD_CONTENT_LIMIT {
        return content.to_string();
    }

    let prefix = format!(
        "**Hatena Bookmark Threshold Item**\n\n**Threshold:** {threshold} bookmarks\n**Bookmarks:** {bookmark_count}\n**Title:** "
    );
    let suffix = format!("\n**Link:** {link}");
    let available = DISCORD_CONTENT_LIMIT
        .saturating_sub(prefix.chars().count())
        .saturating_sub(suffix.chars().count());
    let title = truncate("Title was too long after formatting.", available);

    format!("{prefix}{title}{suffix}")
}

#[cfg(test)]
mod tests {
    use super::{DISCORD_CONTENT_LIMIT, format_threshold_message};
    use crate::state::ArticleSnapshot;

    fn article_with_title(title: &str) -> ArticleSnapshot {
        ArticleSnapshot {
            article_id: "article-1".to_string(),
            title: title.to_string(),
            url: "https://example.com/item".to_string(),
        }
    }

    #[test]
    fn threshold_message_includes_threshold_count_and_link() {
        let message = format_threshold_message(&article_with_title("Title"), 5, 12);

        assert!(message.contains("**Threshold:** 5 bookmarks"));
        assert!(message.contains("**Bookmarks:** 12"));
        assert!(message.contains("**Title:** Title"));
        assert!(message.contains("**Link:** https://example.com/item"));
    }

    #[test]
    fn threshold_message_uses_threshold_specific_labels() {
        let article = article_with_title("Title");

        assert!(format_threshold_message(&article, 1, 1).contains("New Hatena Bookmark Item"));
        assert!(format_threshold_message(&article, 5, 5).contains("Rising"));
        assert!(format_threshold_message(&article, 20, 20).contains("Hot"));
    }

    #[test]
    fn threshold_message_disables_mentions() {
        let message = format_threshold_message(&article_with_title("@everyone <@123>"), 1, 1);

        assert!(!message.contains("@everyone"));
        assert!(!message.contains("<@123>"));
    }

    #[test]
    fn long_threshold_message_stays_under_limit_and_keeps_link() {
        let message = format_threshold_message(&article_with_title(&"title ".repeat(1000)), 20, 25);

        assert!(message.chars().count() <= DISCORD_CONTENT_LIMIT);
        assert!(message.contains("https://example.com/item"));
    }
}
