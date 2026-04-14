mod chafa;
mod gif_anim;
mod open_external;

pub use chafa::chafa_from_bytes;
pub use gif_anim::decode_gif_animation;
pub use open_external::{open_file_path, write_temp_video_bytes};

use crate::api::types::{
    EmbedMediaResponse, MessageAttachmentResponse, MessageEmbedResponse, MessageResponse,
};

fn pick_media_url(m: &EmbedMediaResponse) -> Option<String> {
    m.proxy_url
        .clone()
        .or_else(|| m.url.clone())
        .filter(|u| u.starts_with("http://") || u.starts_with("https://"))
}

fn attachment_is_probably_image(a: &MessageAttachmentResponse) -> bool {
    let mime = a.content_type.as_deref().unwrap_or("");
    if mime.starts_with("image/") {
        return true;
    }
    let n = a.filename.to_lowercase();
    n.ends_with(".png")
        || n.ends_with(".jpg")
        || n.ends_with(".jpeg")
        || n.ends_with(".gif")
        || n.ends_with(".webp")
        || n.ends_with(".bmp")
        || n.ends_with(".avif")
}

fn attachment_is_probably_video(a: &MessageAttachmentResponse) -> bool {
    let mime = a.content_type.as_deref().unwrap_or("");
    if mime.starts_with("video/") {
        return true;
    }
    let n = a.filename.to_lowercase();
    n.ends_with(".mp4")
        || n.ends_with(".webm")
        || n.ends_with(".mov")
        || n.ends_with(".mkv")
        || n.ends_with(".avi")
        || n.ends_with(".m4v")
        || n.ends_with(".ogv")
}

pub fn attachment_image_url(a: &MessageAttachmentResponse) -> Option<String> {
    if !attachment_is_probably_image(a) {
        return None;
    }
    a.proxy_url
        .clone()
        .or_else(|| a.url.clone())
        .filter(|u| u.starts_with("http://") || u.starts_with("https://"))
}

fn attachment_video_url(a: &MessageAttachmentResponse) -> Option<String> {
    if !attachment_is_probably_video(a) {
        return None;
    }
    a.proxy_url
        .clone()
        .or_else(|| a.url.clone())
        .filter(|u| u.starts_with("http://") || u.starts_with("https://"))
}

pub fn embed_image_url(embed: &MessageEmbedResponse) -> Option<String> {
    embed
        .image
        .as_ref()
        .and_then(pick_media_url)
        .or_else(|| embed.thumbnail.as_ref().and_then(pick_media_url))
}

fn embed_direct_media_url(embed: &MessageEmbedResponse) -> Option<String> {
    let t = embed.embed_type.as_str();
    if matches!(t, "image" | "gifv" | "video") {
        return embed
            .image
            .as_ref()
            .and_then(|m| m.proxy_url.clone().or_else(|| m.url.clone()))
            .or_else(|| embed.url.clone())
            .filter(|u| u.starts_with("http://") || u.starts_with("https://"));
    }
    None
}

fn embed_label(embed: &MessageEmbedResponse, fallback: &str) -> String {
    embed
        .title
        .clone()
        .filter(|t| !t.trim().is_empty())
        .unwrap_or_else(|| fallback.to_string())
}

#[derive(Debug, Clone)]
pub enum MessagePreviewMedia {
    Image { url: String, label: String },
    Video { url: String, label: String },
}

/// First image or video suitable for Ctrl+O (images preview in-terminal; videos open externally).
pub fn first_message_preview_media(msg: &MessageResponse) -> Option<MessagePreviewMedia> {
    for a in &msg.attachments {
        if let Some(u) = attachment_image_url(a) {
            let label = if a.filename.is_empty() {
                "image".to_string()
            } else {
                a.filename.clone()
            };
            return Some(MessagePreviewMedia::Image { url: u, label });
        }
    }
    for a in &msg.attachments {
        if let Some(u) = attachment_video_url(a) {
            let label = if a.filename.is_empty() {
                "video".to_string()
            } else {
                a.filename.clone()
            };
            return Some(MessagePreviewMedia::Video { url: u, label });
        }
    }

    for e in &msg.embeds {
        let t = e.embed_type.as_str();
        if matches!(t, "video" | "gifv") {
            if let Some(u) = embed_direct_media_url(e) {
                return Some(MessagePreviewMedia::Video {
                    url: u,
                    label: embed_label(e, "video"),
                });
            }
        }
    }

    for e in &msg.embeds {
        if let Some(u) = embed_image_url(e).or_else(|| embed_direct_media_url(e)) {
            return Some(MessagePreviewMedia::Image {
                url: u,
                label: embed_label(e, "embed image"),
            });
        }
    }

    None
}

// Caught you
// Sniffing my boxers
// Who the fuck does that at Red Lobster ?
// Creepy
// Like when Tom Cruise laughs
// That's how your finger
// Felt in my ass
