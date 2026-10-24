use std::fs::{self, File};
use std::io::BufReader;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::{Duration, Instant};

use crate::domain::asset::{MediaKind, MetadataStatus, ThumbnailStatus};
use crate::errors::{AppError, AppResult};

const THUMBNAIL_MAX_EDGE: u32 = 480;
const THUMBNAIL_JPEG_QUALITY: u8 = 80;
/// 超过该体积的图片不生成缩略图，但仍会尝试读取尺寸。
const MAX_THUMBNAIL_SOURCE_BYTES: u64 = 120 * 1024 * 1024;
const FFMPEG_TIMEOUT: Duration = Duration::from_secs(20);

#[derive(Debug, Clone)]
pub struct ThumbnailResult {
    pub status: ThumbnailStatus,
    pub relative_path: Option<String>,
    pub error: Option<String>,
}

impl ThumbnailResult {
    fn unsupported(message: &str) -> Self {
        Self {
            status: ThumbnailStatus::Unsupported,
            relative_path: None,
            error: Some(message.to_string()),
        }
    }

    fn failed(message: String) -> Self {
        Self {
            status: ThumbnailStatus::Failed,
            relative_path: None,
            error: Some(message),
        }
    }

    fn ready(relative_path: String) -> Self {
        Self {
            status: ThumbnailStatus::Ready,
            relative_path: Some(relative_path),
            error: None,
        }
    }
}

#[derive(Debug, Clone)]
pub struct ProbeResult {
    pub status: MetadataStatus,
    pub width: Option<i64>,
    pub height: Option<i64>,
    pub duration_ms: Option<i64>,
    pub frame_rate: Option<f64>,
    pub sample_rate: Option<i64>,
    pub channels: Option<i64>,
    pub codec: Option<String>,
    pub error: Option<String>,
    pub thumbnail: ThumbnailResult,
}

impl ProbeResult {
    pub fn unsupported(reason: &str) -> Self {
        Self {
            status: MetadataStatus::Unsupported,
            width: None,
            height: None,
            duration_ms: None,
            frame_rate: None,
            sample_rate: None,
            channels: None,
            codec: None,
            error: Some(reason.to_string()),
            thumbnail: ThumbnailResult::unsupported(reason),
        }
    }

    pub fn failed(reason: String) -> Self {
        Self {
            status: MetadataStatus::Failed,
            width: None,
            height: None,
            duration_ms: None,
            frame_rate: None,
            sample_rate: None,
            channels: None,
            codec: None,
            error: Some(reason.clone()),
            thumbnail: ThumbnailResult::failed(reason),
        }
    }
}

/// 媒体探测服务：只读素材文件，缩略图写入应用缓存目录。
pub struct MediaService {
    cache_root: PathBuf,
    ffmpeg: Option<PathBuf>,
}

impl MediaService {
    pub fn new(cache_root: PathBuf) -> Self {
        let ffmpeg = find_ffmpeg();
        if let Some(path) = &ffmpeg {
            tracing::info!(path = %path.display(), "检测到 ffmpeg，可生成视频封面");
        } else {
            tracing::info!("未检测到 ffmpeg，视频封面生成不可用");
        }
        Self { cache_root, ffmpeg }
    }

    pub fn cache_root(&self) -> &Path {
        &self.cache_root
    }

    pub fn ffmpeg_available(&self) -> bool {
        self.ffmpeg.is_some()
    }

    pub fn ffmpeg_path(&self) -> Option<&Path> {
        self.ffmpeg.as_deref()
    }

    /// 探测素材并生成缩略图（如支持）。失败不返回 Err，而是写入状态与原因。
    pub fn probe(
        &self,
        asset_id: &str,
        work_id: &str,
        path: &Path,
        media_kind: MediaKind,
    ) -> ProbeResult {
        if !path.is_file() {
            return ProbeResult::failed("文件不存在，无法读取媒体信息".to_string());
        }

        match media_kind {
            MediaKind::Image => self.probe_image(asset_id, work_id, path),
            MediaKind::Audio => self.probe_audio(asset_id, work_id, path),
            MediaKind::Video => self.probe_video(asset_id, work_id, path),
            MediaKind::Document | MediaKind::Other => {
                ProbeResult::unsupported("该类型不提取媒体信息")
            }
        }
    }

    fn extension(path: &Path) -> String {
        path.extension()
            .and_then(|value| value.to_str())
            .unwrap_or_default()
            .to_ascii_lowercase()
    }

    fn probe_image(&self, asset_id: &str, work_id: &str, path: &Path) -> ProbeResult {
        if Self::extension(path) == "svg" {
            return ProbeResult::unsupported("SVG 暂不支持尺寸提取与缩略图");
        }

        let reader = match image::ImageReader::open(path) {
            Ok(reader) => reader,
            Err(error) => {
                return ProbeResult::failed(format!("无法打开图片：{error}"));
            }
        };
        let reader = match reader.with_guessed_format() {
            Ok(reader) => reader,
            Err(error) => {
                return ProbeResult::failed(format!("无法识别图片格式：{error}"));
            }
        };
        let (width, height) = match reader.into_dimensions() {
            Ok(size) => size,
            Err(error) => {
                return ProbeResult::failed(format!("无法解析图片尺寸：{error}"));
            }
        };

        let thumbnail = self.generate_image_thumbnail(asset_id, work_id, path);
        ProbeResult {
            status: MetadataStatus::Ready,
            width: Some(width as i64),
            height: Some(height as i64),
            duration_ms: None,
            frame_rate: None,
            sample_rate: None,
            channels: None,
            codec: Some(Self::extension(path)),
            error: None,
            thumbnail,
        }
    }

    fn generate_image_thumbnail(
        &self,
        asset_id: &str,
        work_id: &str,
        path: &Path,
    ) -> ThumbnailResult {
        let size = match fs::metadata(path) {
            Ok(metadata) => metadata.len(),
            Err(error) => return ThumbnailResult::failed(format!("无法读取文件大小：{error}")),
        };
        if size > MAX_THUMBNAIL_SOURCE_BYTES {
            return ThumbnailResult::unsupported("图片过大，未生成缩略图");
        }

        let image = match image::open(path) {
            Ok(image) => image,
            Err(error) => return ThumbnailResult::failed(format!("无法解码图片：{error}")),
        };
        let thumbnail = image.thumbnail(THUMBNAIL_MAX_EDGE, THUMBNAIL_MAX_EDGE);

        match self.write_thumbnail(asset_id, work_id, thumbnail) {
            Ok(relative) => ThumbnailResult::ready(relative),
            Err(error) => ThumbnailResult::failed(error.message),
        }
    }

    fn probe_audio(&self, asset_id: &str, work_id: &str, path: &Path) -> ProbeResult {
        use lofty::file::AudioFile;
        use lofty::probe::Probe;

        let tagged = match Probe::open(path).and_then(|probe| probe.read()) {
            Ok(tagged) => tagged,
            Err(error) => return ProbeResult::failed(format!("无法读取音频信息：{error}")),
        };
        let properties = tagged.properties();
        let codec = Self::extension(path).to_uppercase();

        let thumbnail = self.embedded_cover(asset_id, work_id, &tagged);

        ProbeResult {
            status: MetadataStatus::Ready,
            width: None,
            height: None,
            duration_ms: Some(properties.duration().as_millis() as i64),
            frame_rate: None,
            sample_rate: properties.sample_rate().map(|value| value as i64),
            channels: properties.channels().map(|value| value as i64),
            codec: Some(codec),
            error: None,
            thumbnail,
        }
    }

    fn embedded_cover(
        &self,
        asset_id: &str,
        work_id: &str,
        tagged: &lofty::file::TaggedFile,
    ) -> ThumbnailResult {
        use lofty::file::TaggedFileExt;
        use lofty::picture::MimeType;

        let Some(tag) = tagged.primary_tag().or_else(|| tagged.first_tag()) else {
            return ThumbnailResult::unsupported("音频没有内嵌封面");
        };
        let Some(picture) = tag.pictures().first() else {
            return ThumbnailResult::unsupported("音频没有内嵌封面");
        };
        if !matches!(
            picture.mime_type(),
            Some(MimeType::Jpeg) | Some(MimeType::Png)
        ) {
            return ThumbnailResult::unsupported("音频封面格式不受支持");
        }

        let image = match image::load_from_memory(picture.data()) {
            Ok(image) => image,
            Err(error) => return ThumbnailResult::failed(format!("无法解码音频封面：{error}")),
        };
        let thumbnail = image.thumbnail(THUMBNAIL_MAX_EDGE, THUMBNAIL_MAX_EDGE);
        match self.write_thumbnail(asset_id, work_id, thumbnail) {
            Ok(relative) => ThumbnailResult::ready(relative),
            Err(error) => ThumbnailResult::failed(error.message),
        }
    }

    fn probe_video(&self, asset_id: &str, work_id: &str, path: &Path) -> ProbeResult {
        let extension = Self::extension(path);
        if !matches!(extension.as_str(), "mp4" | "m4v" | "mov") {
            return ProbeResult::unsupported("当前仅解析 MP4/MOV，其他格式需外部媒体工具");
        }

        let size = match fs::metadata(path) {
            Ok(metadata) => metadata.len(),
            Err(error) => return ProbeResult::failed(format!("无法读取视频大小：{error}")),
        };
        if size < 8 {
            return ProbeResult::failed("文件为空或不是有效的 MP4".to_string());
        }

        let file = match File::open(path) {
            Ok(file) => file,
            Err(error) => return ProbeResult::failed(format!("无法打开视频：{error}")),
        };
        let reader = BufReader::new(file);
        let mp4 = match mp4::Mp4Reader::read_header(reader, size) {
            Ok(mp4) => mp4,
            Err(error) => return ProbeResult::failed(format!("无法解析 MP4：{error}")),
        };

        let duration_ms = Some(mp4.duration().as_millis() as i64);
        let mut width = None;
        let mut height = None;
        let mut frame_rate = None;
        let mut codec = None;

        for track in mp4.tracks().values() {
            let is_video = matches!(track.track_type(), Ok(mp4::TrackType::Video));
            if !is_video {
                continue;
            }
            width = Some(track.width() as i64);
            height = Some(track.height() as i64);
            codec = track.media_type().ok().map(|value| format!("{value:?}"));
            let seconds = track.duration().as_secs_f64();
            let samples = track.sample_count();
            if seconds > 0.0 && samples > 0 {
                frame_rate = Some(f64::from(samples) / seconds);
            }
            break;
        }

        let thumbnail = self.video_thumbnail(asset_id, work_id, path);
        ProbeResult {
            status: if width.is_some() {
                MetadataStatus::Ready
            } else {
                MetadataStatus::Partial
            },
            width,
            height,
            duration_ms,
            frame_rate,
            sample_rate: None,
            channels: None,
            codec,
            error: if width.is_none() {
                Some("未找到视频轨道".to_string())
            } else {
                None
            },
            thumbnail,
        }
    }

    fn video_thumbnail(&self, asset_id: &str, work_id: &str, path: &Path) -> ThumbnailResult {
        let Some(ffmpeg) = &self.ffmpeg else {
            return ThumbnailResult::unsupported(
                "未找到 ffmpeg，视频封面暂不可用（可将 ffmpeg.exe 放到程序目录或 PATH）",
            );
        };

        let target = match self.thumbnail_target(asset_id, work_id, path) {
            Ok(target) => target,
            Err(error) => return ThumbnailResult::failed(error.message),
        };
        if let Some(parent) = target.parent() {
            if let Err(error) = fs::create_dir_all(parent) {
                return ThumbnailResult::failed(format!("无法创建缩略图目录：{error}"));
            }
        }
        let _ = fs::remove_file(&target);

        // 先尝试第 1 秒；短视频或关键帧较晚时回退到开头，避免直接失败
        let mut last_error = String::new();
        for seek in ["1", "0"] {
            let mut command = Command::new(ffmpeg);
            command
                .arg("-hide_banner")
                .arg("-loglevel")
                .arg("error")
                .arg("-ss")
                .arg(seek)
                .arg("-i")
                .arg(path)
                .arg("-frames:v")
                .arg("1")
                .arg("-vf")
                .arg(format!(
                    "scale='min({THUMBNAIL_MAX_EDGE},iw)':-2:flags=lanczos"
                ))
                .arg("-q:v")
                .arg("4")
                .arg("-y")
                .arg(&target);
            #[cfg(windows)]
            {
                use std::os::windows::process::CommandExt;
                const CREATE_NO_WINDOW: u32 = 0x0800_0000;
                command.creation_flags(CREATE_NO_WINDOW);
            }

            match run_with_timeout(&mut command, FFMPEG_TIMEOUT) {
                Ok(()) if target.is_file() => {
                    return ThumbnailResult::ready(self.relative_thumbnail_path(&target));
                }
                Ok(()) => last_error = "ffmpeg 未生成封面文件".to_string(),
                Err(reason) => last_error = reason,
            }
        }
        ThumbnailResult::failed(last_error)
    }

    fn thumbnail_target(&self, asset_id: &str, work_id: &str, source: &Path) -> AppResult<PathBuf> {
        let metadata = fs::metadata(source)
            .map_err(|error| AppError::io_context("无法读取素材信息", error))?;
        let modified = metadata
            .modified()
            .ok()
            .and_then(|time| time.duration_since(std::time::UNIX_EPOCH).ok())
            .map(|duration| duration.as_secs())
            .unwrap_or(0);
        let file_name = format!("{asset_id}-{}-{modified}.jpg", metadata.len());
        Ok(self.cache_root.join(work_id).join(file_name))
    }

    fn write_thumbnail(
        &self,
        asset_id: &str,
        work_id: &str,
        thumbnail: image::DynamicImage,
    ) -> AppResult<String> {
        // 缓存文件名包含源文件大小与修改时间，源文件变化时自然失效
        let source_stamp = format!("{}-{}", thumbnail.width(), thumbnail.height());
        let file_name = format!("{asset_id}-{source_stamp}.jpg");
        let target = self.cache_root.join(work_id).join(file_name);
        if let Some(parent) = target.parent() {
            fs::create_dir_all(parent)
                .map_err(|error| AppError::io_context("无法创建缩略图目录", error))?;
        }

        let rgb = thumbnail.to_rgb8();
        let file =
            File::create(&target).map_err(|error| AppError::io_context("无法写入缩略图", error))?;
        let encoder =
            image::codecs::jpeg::JpegEncoder::new_with_quality(file, THUMBNAIL_JPEG_QUALITY);
        image::DynamicImage::ImageRgb8(rgb)
            .write_with_encoder(encoder)
            .map_err(|error| AppError::internal(format!("缩略图编码失败：{error}")))?;

        self.cleanup_stale_thumbnails(asset_id, work_id, &target);
        Ok(self.relative_thumbnail_path(&target))
    }

    fn relative_thumbnail_path(&self, target: &Path) -> String {
        target
            .strip_prefix(&self.cache_root)
            .map(|path| path.to_string_lossy().replace('\\', "/"))
            .unwrap_or_else(|_| target.to_string_lossy().replace('\\', "/"))
    }

    fn cleanup_stale_thumbnails(&self, asset_id: &str, work_id: &str, keep: &Path) {
        let directory = self.cache_root.join(work_id);
        let Ok(entries) = fs::read_dir(&directory) else {
            return;
        };
        for entry in entries.flatten() {
            let path = entry.path();
            if path == keep {
                continue;
            }
            let matches_asset = path
                .file_name()
                .and_then(|name| name.to_str())
                .map(|name| name.starts_with(&format!("{asset_id}-")))
                .unwrap_or(false);
            if matches_asset {
                let _ = fs::remove_file(&path);
            }
        }
    }
}

fn find_ffmpeg() -> Option<PathBuf> {
    if let Ok(custom) = std::env::var("CREATORFLOW_FFMPEG") {
        let path = PathBuf::from(custom);
        if path.is_file() {
            return Some(path);
        }
    }

    let executable_name = if cfg!(windows) {
        "ffmpeg.exe"
    } else {
        "ffmpeg"
    };
    if let Ok(executable) = std::env::current_exe() {
        if let Some(directory) = executable.parent() {
            let candidate = directory.join(executable_name);
            if candidate.is_file() {
                return Some(candidate);
            }
        }
    }
    let search_path = std::env::var_os("PATH")?;
    std::env::split_paths(&search_path)
        .map(|directory| directory.join(executable_name))
        .find(|candidate| candidate.is_file())
}

fn run_with_timeout(command: &mut Command, timeout: Duration) -> Result<(), String> {
    let mut child = command
        .spawn()
        .map_err(|error| format!("无法启动 ffmpeg：{error}"))?;
    let started = Instant::now();
    loop {
        match child.try_wait() {
            Ok(Some(status)) if status.success() => return Ok(()),
            Ok(Some(status)) => {
                return Err(format!(
                    "ffmpeg 执行失败（退出码 {}）",
                    status.code().unwrap_or(-1)
                ))
            }
            Ok(None) => {
                if started.elapsed() > timeout {
                    let _ = child.kill();
                    return Err("ffmpeg 执行超时".to_string());
                }
                std::thread::sleep(Duration::from_millis(50));
            }
            Err(error) => return Err(format!("无法等待 ffmpeg：{error}")),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    fn write_png(path: &Path, width: u32, height: u32) {
        let image = image::RgbImage::from_fn(width, height, |x, y| {
            image::Rgb([(x % 255) as u8, (y % 255) as u8, 128])
        });
        image::DynamicImage::ImageRgb8(image)
            .save(path)
            .expect("写入测试图片失败");
    }

    #[test]
    fn probes_image_and_writes_thumbnail() {
        let temp = TempDir::new().unwrap();
        let cache = temp.path().join("cache");
        let service = MediaService::new(cache.clone());
        let image_path = temp.path().join("cover.png");
        write_png(&image_path, 64, 32);

        let result = service.probe("asset-1", "work-1", &image_path, MediaKind::Image);
        assert_eq!(result.status, MetadataStatus::Ready);
        assert_eq!(result.width, Some(64));
        assert_eq!(result.height, Some(32));
        assert_eq!(result.thumbnail.status, ThumbnailStatus::Ready);
        let relative = result.thumbnail.relative_path.expect("应生成缩略图");
        assert!(cache.join(relative).is_file());
    }

    #[test]
    fn corrupt_image_is_reported_without_panicking() {
        let temp = TempDir::new().unwrap();
        let service = MediaService::new(temp.path().join("cache"));
        let path = temp.path().join("broken.png");
        fs::write(&path, b"not an image").unwrap();

        let result = service.probe("asset-2", "work-1", &path, MediaKind::Image);
        assert_eq!(result.status, MetadataStatus::Failed);
        assert!(result.error.is_some());
    }

    #[test]
    fn probe_missing_file_returns_failed() {
        let temp = TempDir::new().unwrap();
        let service = MediaService::new(temp.path().join("cache"));
        let result = service.probe(
            "asset-3",
            "work-1",
            &temp.path().join("missing.mp4"),
            MediaKind::Video,
        );
        assert_eq!(result.status, MetadataStatus::Failed);
    }

    #[test]
    fn unsupported_video_extension_is_marked_unsupported() {
        let temp = TempDir::new().unwrap();
        let service = MediaService::new(temp.path().join("cache"));
        let path = temp.path().join("clip.mkv");
        fs::write(&path, b"fake").unwrap();
        let result = service.probe("asset-4", "work-1", &path, MediaKind::Video);
        assert_eq!(result.status, MetadataStatus::Unsupported);
        assert_eq!(result.thumbnail.status, ThumbnailStatus::Unsupported);
    }
}
