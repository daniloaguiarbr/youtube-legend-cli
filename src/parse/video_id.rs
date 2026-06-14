//! Extract and validate the 11-character `YouTube` video id from any
//! recognized `YouTube` URL form.

use crate::error::{AppError, AppResult};

/// Parse a `YouTube` URL and return the 11-character video id.
///
/// Supports the following URL forms:
///
/// - `https://www.youtube.com/watch?v=<id>`
/// - `https://youtube.com/watch?v=<id>`
/// - `https://m.youtube.com/watch?v=<id>`
/// - `https://www.youtube.com/shorts/<id>`
/// - `https://www.youtube.com/embed/<id>`
/// - `https://youtu.be/<id>`
///
/// # Errors
///
/// - [`AppError::InvalidUrl`] if the input is not a syntactically valid URL,
///   does not point to a `YouTube` host, or does not carry a video id in
///   any of the supported forms, or if the extracted id is not exactly 11
///   characters of `[A-Za-z0-9_-]`.
///
/// # Examples
///
/// ```
/// use youtube_legend_cli::parse::video_id::extract_video_id;
///
/// let id = extract_video_id("https://youtu.be/dQw4w9WgXcQ").unwrap();
/// assert_eq!(id, "dQw4w9WgXcQ");
/// ```
#[tracing::instrument(level = "debug", err, skip(input), fields(input = %input.chars().take(64).collect::<String>()))]
pub fn extract_video_id(input: &str) -> AppResult<String> {
    let url = url::Url::parse(input)
        .map_err(|e| AppError::InvalidUrl(format!("could not parse url: {e}")))?;

    let host = url
        .host_str()
        .ok_or_else(|| AppError::InvalidUrl("url has no host".to_string()))?
        .to_lowercase();

    let path_segments: Vec<&str> = url
        .path_segments()
        .map(|s| s.filter(|p| !p.is_empty()).collect())
        .unwrap_or_default();

    let video_id = match host.as_str() {
        "youtu.be" => path_segments
            .first()
            .copied()
            .ok_or_else(|| AppError::InvalidUrl("youtu.be url has no id".to_string()))?
            .to_string(),
        "youtube.com" | "www.youtube.com" | "m.youtube.com" => {
            if let Some(query_id) = url.query_pairs().find(|(k, _)| k == "v") {
                query_id.1.into_owned()
            } else if path_segments.first().copied() == Some("shorts")
                || path_segments.first().copied() == Some("embed")
            {
                path_segments
                    .get(1)
                    .copied()
                    .ok_or_else(|| {
                        AppError::InvalidUrl(format!(
                            "{} path has no id",
                            path_segments.first().copied().unwrap_or("path")
                        ))
                    })?
                    .to_string()
            } else {
                return Err(AppError::InvalidUrl(
                    "youtube.com url has neither ?v= nor /shorts/ nor /embed/".to_string(),
                ));
            }
        }
        _ => {
            return Err(AppError::InvalidUrl(format!(
                "domain is not youtube: {host}"
            )));
        }
    };

    validate_video_id(&video_id)?;
    Ok(video_id)
}

fn validate_video_id(id: &str) -> AppResult<()> {
    if id.len() != 11 {
        return Err(AppError::InvalidUrl(format!(
            "video id must be 11 characters, got {}: {id}",
            id.len()
        )));
    }
    if !id
        .chars()
        .all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '-')
    {
        return Err(AppError::InvalidUrl(format!(
            "video id contains invalid characters: {id}"
        )));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn extracts_watch_url() {
        let id = extract_video_id("https://www.youtube.com/watch?v=dQw4w9WgXcQ").unwrap();
        assert_eq!(id, "dQw4w9WgXcQ");
    }

    #[test]
    fn extracts_short_url() {
        let id = extract_video_id("https://youtu.be/dQw4w9WgXcQ").unwrap();
        assert_eq!(id, "dQw4w9WgXcQ");
    }

    #[test]
    fn extracts_shorts_url() {
        let id = extract_video_id("https://www.youtube.com/shorts/dQw4w9WgXcQ").unwrap();
        assert_eq!(id, "dQw4w9WgXcQ");
    }

    #[test]
    fn extracts_embed_url() {
        let id = extract_video_id("https://www.youtube.com/embed/dQw4w9WgXcQ").unwrap();
        assert_eq!(id, "dQw4w9WgXcQ");
    }

    #[test]
    fn rejects_non_youtube() {
        let err = extract_video_id("https://example.com/watch?v=ABC123DEF45").unwrap_err();
        assert!(matches!(err, AppError::InvalidUrl(_)));
    }

    #[test]
    fn rejects_short_id() {
        let err = extract_video_id("https://youtu.be/abc").unwrap_err();
        assert!(matches!(err, AppError::InvalidUrl(_)));
    }

    #[test]
    fn rejects_special_chars() {
        let err = extract_video_id("https://www.youtube.com/watch?v=abc!@#$%^&*()").unwrap_err();
        assert!(matches!(err, AppError::InvalidUrl(_)));
    }

    #[test]
    fn rejects_invalid_url() {
        let err = extract_video_id("not a url at all").unwrap_err();
        assert!(matches!(err, AppError::InvalidUrl(_)));
    }
}
