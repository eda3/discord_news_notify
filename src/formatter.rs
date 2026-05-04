use crate::rss::RssItem;

pub const DISCORD_CONTENT_LIMIT: usize = 2000;

const TITLE_LIMIT: usize = 200;
const DESCRIPTION_LIMIT: usize = 1200;
const FEED_TITLE_LIMIT: usize = 120;
const LINK_LIMIT: usize = 500;
const ELLIPSIS: &str = "...";

pub fn format_item_message(item: &RssItem) -> String {
    let title = clean_and_truncate(&item.title, TITLE_LIMIT, "No Title");
    let description = clean_and_truncate(
        &item.description,
        DESCRIPTION_LIMIT,
        "No description available",
    );
    let pub_date = item
        .pub_date
        .map(|date| date.to_rfc3339())
        .unwrap_or_else(|| "Unknown".to_string());
    let link = if item.link.trim().is_empty() {
        "No link".to_string()
    } else {
        truncate(item.link.trim(), LINK_LIMIT)
    };
    let feed_title = item
        .feed_title
        .as_deref()
        .map(|value| clean_and_truncate(value, FEED_TITLE_LIMIT, "Unknown feed"))
        .unwrap_or_else(|| "Unknown feed".to_string());

    let content = format!(
        "**New RSS Item**\n\n**Feed:** {feed_title}\n**Title:** {title}\n**Description:** {description}\n**Published:** {pub_date}\n**Link:** {link}",
    );

    truncate_preserving_link(&content, &link)
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

fn truncate_preserving_link(content: &str, link: &str) -> String {
    if content.chars().count() <= DISCORD_CONTENT_LIMIT {
        return content.to_string();
    }

    let prefix = "**New RSS Item**\n\n**Description:** ";
    let suffix = format!("\n**Link:** {link}");
    let available = DISCORD_CONTENT_LIMIT
        .saturating_sub(prefix.chars().count())
        .saturating_sub(suffix.chars().count());
    let description = truncate("Message was too long after formatting.", available);

    format!("{prefix}{description}{suffix}")
}

#[cfg(test)]
mod tests {
    use super::{DISCORD_CONTENT_LIMIT, format_item_message};
    use crate::rss::RssItem;

    fn item_with_description(description: &str) -> RssItem {
        RssItem {
            guid: Some("guid-1".to_string()),
            title: "Title".to_string(),
            link: "https://example.com/item".to_string(),
            description: description.to_string(),
            pub_date: None,
            feed_title: Some("Example Feed".to_string()),
        }
    }

    #[test]
    fn message_includes_expected_fields() {
        let message = format_item_message(&item_with_description("Description"));

        assert!(message.contains("**Feed:** Example Feed"));
        assert!(message.contains("**Title:** Title"));
        assert!(message.contains("**Description:** Description"));
        assert!(message.contains("**Published:** Unknown"));
        assert!(message.contains("**Link:** https://example.com/item"));
    }

    #[test]
    fn message_strips_html_and_normalizes_whitespace() {
        let message = format_item_message(&item_with_description("<p>Hello<br> world</p>"));

        assert!(message.contains("Hello world"));
        assert!(!message.contains("<p>"));
    }

    #[test]
    fn message_disables_mentions_in_content() {
        let message = format_item_message(&item_with_description("@everyone @here <@123> <@&456>"));

        assert!(!message.contains("@everyone"));
        assert!(!message.contains("@here"));
        assert!(!message.contains("<@123>"));
        assert!(!message.contains("<@&456>"));
    }

    #[test]
    fn long_message_stays_under_discord_limit_and_keeps_link() {
        let message = format_item_message(&item_with_description(&"long ".repeat(1000)));

        assert!(message.chars().count() <= DISCORD_CONTENT_LIMIT);
        assert!(message.contains("https://example.com/item"));
    }

    #[test]
    fn long_title_is_truncated() {
        let item = RssItem {
            guid: Some("guid-1".to_string()),
            title: "title ".repeat(100),
            link: "https://example.com/item".to_string(),
            description: "Description".to_string(),
            pub_date: None,
            feed_title: Some("Example Feed".to_string()),
        };

        let message = format_item_message(&item);

        assert!(message.contains("**Title:** "));
        assert!(message.contains("..."));
        assert!(!message.contains(&"title ".repeat(100)));
    }

    #[test]
    fn empty_title_and_description_use_fallbacks() {
        let item = RssItem {
            guid: Some("guid-1".to_string()),
            title: " ".to_string(),
            link: String::new(),
            description: " ".to_string(),
            pub_date: None,
            feed_title: None,
        };

        let message = format_item_message(&item);

        assert!(message.contains("**Title:** No Title"));
        assert!(message.contains("**Description:** No description available"));
        assert!(message.contains("**Link:** No link"));
    }
}
