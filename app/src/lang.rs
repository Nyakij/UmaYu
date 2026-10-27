//! The app's display language: English (the default) or Vietnamese, picked in Settings →
//! Appearance and saved as `lang` in settings.json (`UMALORD_LANG` overrides it).
//!
//! Every label is written in English in the code, and that English text is the key of the
//! Vietnamese table below: `t!("Mode")` gives "Chế độ" in Vietnamese and "Mode" in English. Text
//! with no entry yet stays English, so the UI never breaks on a missing translation. Text with
//! values goes through `tf!("Saved {}.", path)`: the template is translated whole, then `{}` (in
//! order) or `{0}`, `{1}` (by position, for translations that reorder) are filled in.
//!
//! What stays English in the Vietnamese UI, on purpose:
//! - Game data: names of umas, outfits, support cards, skills, races, courses and events.
//! - Game terms: Speed, Stamina, Power, Guts, Wit; Turf, Dirt; Sprint, Mile, Medium, Long; the
//!   running styles (Front, Pace, Late, End, Front Runner…); Champions Meeting (CM), Team Trials,
//!   League of Heroes (LoH); scenario names (URA Finale, Unity Cup, Grand Concert, Trackblazer…);
//!   Career, trainee, uma, support card, deck, limit break (LB, MLB), potential, aptitude, spark,
//!   parent, hint, SP, skill point, meta, tier, build, preset, roster, profile, server.
//! - Text meant for the AI (agent tool schemas and results, the Ask tab's system prompt), the
//!   digest, the CLI and logs.
//!
//! The app's own words, kept the same everywhere: Home = Trang chủ, Collection = Bộ sưu tập,
//! Career = Career, Ask = Hỏi đáp, Plan = Kế hoạch, Race sim = Mô phỏng đua, Results = Kết quả,
//! Notes = Ghi chú, Settings = Cài đặt, Help = Trợ giúp, Reload = Tải lại; the planner's buttons
//! ✨ Recommend whole deck = ✨ Gợi ý cả deck, 💾 Save plan = 💾 Lưu kế hoạch, ▶ Simulate =
//! ▶ Mô phỏng; Skills to buy = Skill cần mua.

use std::collections::HashMap;
use std::fmt::{Display, Write};
use std::sync::OnceLock;
use std::sync::atomic::{AtomicU8, Ordering};

/// A display language.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub enum Lang {
    #[default]
    En,
    Vi,
}

impl Lang {
    pub const ALL: [Lang; 2] = [Lang::En, Lang::Vi];

    /// The name saved in settings.json and taken by `UMALORD_LANG`.
    pub fn key(self) -> &'static str {
        match self {
            Lang::En => "en",
            Lang::Vi => "vi",
        }
    }

    pub fn from_key(key: &str) -> Option<Lang> {
        Lang::ALL
            .into_iter()
            .find(|l| l.key() == key.trim().to_ascii_lowercase())
    }

    /// The language's own name, as the picker shows it in every language.
    pub fn label(self) -> &'static str {
        match self {
            Lang::En => "English",
            Lang::Vi => "Tiếng Việt",
        }
    }
}

static CURRENT: AtomicU8 = AtomicU8::new(0);

/// The language the UI is drawn in, app-wide (read by code that has no `App`).
pub fn current() -> Lang {
    Lang::ALL[usize::from(CURRENT.load(Ordering::Relaxed)).min(Lang::ALL.len() - 1)]
}

/// Switch the language; the next frame is drawn in it.
pub fn set(lang: Lang) {
    CURRENT.store(
        Lang::ALL.iter().position(|l| *l == lang).unwrap_or(0) as u8,
        Ordering::Relaxed,
    );
}

/// The Vietnamese table, built once from `VI`.
fn vi() -> &'static HashMap<&'static str, &'static str> {
    static MAP: OnceLock<HashMap<&'static str, &'static str>> = OnceLock::new();
    MAP.get_or_init(|| VI.iter().copied().collect())
}

/// `text` in the current language; the English itself when there's no translation.
pub fn tr(text: &str) -> &str {
    match current() {
        Lang::En => text,
        Lang::Vi => vi().get(text).copied().unwrap_or(text),
    }
}

/// Fill a translated template: `{}` takes the next value, `{0}`, `{1}` a value by position.
pub fn fill(template: &str, args: &[&dyn Display]) -> String {
    let mut out = String::with_capacity(template.len() + 16);
    let mut next = 0;
    let mut rest = template;
    while let Some(i) = rest.find('{') {
        out.push_str(&rest[..i]);
        let tail = &rest[i + 1..];
        match tail.find('}') {
            Some(j) if tail[..j].bytes().all(|b| b.is_ascii_digit()) => {
                let index = if j == 0 {
                    next += 1;
                    next - 1
                } else {
                    tail[..j].parse().unwrap_or(usize::MAX)
                };
                if let Some(arg) = args.get(index) {
                    let _ = write!(out, "{arg}");
                }
                rest = &tail[j + 1..];
            }
            _ => {
                out.push('{');
                rest = tail;
            }
        }
    }
    out.push_str(rest);
    out
}

/// `t!("English text")`: the text in the current language.
macro_rules! t {
    ($text:expr) => {
        $crate::lang::tr($text)
    };
}

/// `tf!("Saved {}.", path)`: a translated template filled with values, as a `String`.
macro_rules! tf {
    ($text:expr $(, $arg:expr)* $(,)?) => {
        $crate::lang::fill($crate::lang::tr($text), &[$(&$arg as &dyn std::fmt::Display),*])
    };
}

/// English → Vietnamese. One entry per distinct English text; the same text shares one entry.
const VI: &[(&str, &str)] = &[
    // Settings: the sections.
    ("⚙ Settings", "⚙ Cài đặt"),
    ("Profile", "Hồ sơ"),
    (
        "Whose account this is, and saving, loading or emptying it.",
        "Tài khoản này của ai, và lưu, tải hoặc làm trống nó.",
    ),
    ("Appearance", "Giao diện"),
    (
        "Language, dark or light, the colour theme, Seiun Sky, and the size of everything.",
        "Ngôn ngữ, chế độ tối hoặc sáng, chủ đề màu, Seiun Sky và kích thước của mọi thứ.",
    ),
    ("Data and updates", "Dữ liệu và cập nhật"),
    (
        "Where the game data comes from, how old it is, and how to refresh it.",
        "Dữ liệu game lấy từ đâu, cũ đến mức nào và cách làm mới.",
    ),
    ("Game audio", "Âm thanh game"),
    (
        "Connect to local game files to play live music, jukebox tracks and cuesheets.",
        "Kết nối với file game cục bộ để phát nhạc live, jukebox và cuesheet.",
    ),
    ("AI assistant", "Trợ lý AI"),
    (
        "Who answers on the Ask tab: a free model through OpenCode, or Claude with your key.",
        "Ai trả lời ở tab Hỏi đáp: một model miễn phí qua OpenCode, hoặc Claude bằng key của bạn.",
    ),
    ("About", "Giới thiệu"),
    (
        "Version, credits, license and keyboard shortcuts.",
        "Phiên bản, ghi công, giấy phép và phím tắt.",
    ),
    (
        "Changes save as you make them.",
        "Thay đổi được lưu ngay khi bạn thực hiện.",
    ),
    ("Close (Esc)", "Đóng (Esc)"),
    // Settings → Profile.
    (
        "Renamed the account to {}.",
        "Đã đổi tên tài khoản thành {}.",
    ),
    ("Couldn't save the name: {}", "Không lưu được tên: {}"),
    ("Trainer name", "Tên trainer"),
    (
        "Whose account this is. It's saved in account/umas.md and in profiles you save.",
        "Tài khoản này của ai. Tên được lưu trong account/umas.md và trong các profile bạn lưu.",
    ),
    ("e.g. Sam", "vd. Sam"),
    (
        "Saved when you press Enter or click elsewhere. Home greets you by it.",
        "Lưu khi bạn nhấn Enter hoặc bấm ra chỗ khác. Trang chủ sẽ chào bạn bằng tên này.",
    ),
    (
        "Uma Musume runs separate servers with different content. The game data in this app is for Global.",
        "Uma Musume có nhiều server riêng với nội dung khác nhau. Dữ liệu game trong app này dành cho bản Global.",
    ),
    (
        "The game data, meta sheet and simulator are all for Global.",
        "Dữ liệu game, meta sheet và trình mô phỏng đều dành cho bản Global.",
    ),
    ("Your collection", "Bộ sưu tập của bạn"),
    ("{} umas · {} support cards{}", "{} uma · {} support card{}"),
    (" · last updated {}", " · cập nhật lần cuối {}"),
    ("Data folder", "Thư mục dữ liệu"),
    (
        "Everything the app saves lives here: your account, plans, results and profiles.",
        "Mọi thứ app lưu đều nằm ở đây: tài khoản, kế hoạch, kết quả và profile của bạn.",
    ),
    ("📂 Open folder", "📂 Mở thư mục"),
    (
        "The UmaYu folder wasn't found.",
        "Không tìm thấy thư mục UmaYu.",
    ),
    ("Your data", "Dữ liệu của bạn"),
    ("📤 Save my data", "📤 Lưu dữ liệu của tôi"),
    (
        "Your umas, cards, parents, plans and results in one .umalord.json file, to keep or share.",
        "Uma, card, parent, kế hoạch và kết quả của bạn trong một file .umalord.json để giữ lại hoặc chia sẻ.",
    ),
    ("Saved {}.", "Đã lưu {}."),
    ("Show", "Hiện"),
    ("📥 Load a profile", "📥 Tải một profile"),
    (
        "Switch to a saved account. What's here is backed up first.",
        "Chuyển sang một tài khoản đã lưu. Dữ liệu hiện tại sẽ được sao lưu trước.",
    ),
    ("📝 Set up my collection", "📝 Thiết lập bộ sưu tập"),
    (
        "Click the umas and cards you own, and set stars and limit breaks.",
        "Bấm chọn các uma và card bạn sở hữu, rồi đặt số sao và limit break.",
    ),
    ("Danger zone", "Vùng nguy hiểm"),
    ("⚠ Start over…", "⚠ Làm lại từ đầu…"),
    ("Start over", "Làm lại từ đầu"),
    (
        "Back this account up to profiles/backups, then empty the app for a new one. You'll be asked to confirm.",
        "Sao lưu tài khoản này vào profiles/backups, rồi làm trống app cho tài khoản mới. Bạn sẽ được hỏi xác nhận.",
    ),
    // Settings → Appearance.
    ("Language", "Ngôn ngữ"),
    (
        "The language of the app's labels and messages.",
        "Ngôn ngữ của nhãn và thông báo trong app.",
    ),
    ("Mode", "Chế độ"),
    ("◐ System", "◐ Theo hệ thống"),
    ("☀ Light", "☀ Sáng"),
    ("🌙 Dark", "🌙 Tối"),
    (
        "Follow Windows: dark when Windows is dark",
        "Theo Windows: tối khi Windows để chế độ tối",
    ),
    ("Theme", "Chủ đề"),
    (
        "The accent colour of buttons and highlights, and the background. Click one to try it.",
        "Màu nhấn của nút và phần làm nổi bật, cùng hình nền. Bấm vào một chủ đề để thử.",
    ),
    ("Sunset", "Hoàng hôn"),
    ("Starry night", "Đêm sao"),
    ("Track day", "Ngày đua"),
    ("Ink", "Mực"),
    (
        "Sky blue, with soft clouds",
        "Xanh da trời, với những đám mây mềm",
    ),
    (
        "Cherry-blossom pink, with falling petals",
        "Hồng hoa anh đào, với cánh hoa rơi",
    ),
    (
        "Warm orange, with an evening sun",
        "Cam ấm, với mặt trời chiều",
    ),
    ("Violet, with stars and a moon", "Tím, với sao và trăng"),
    (
        "Trophy gold, with the rails of a racetrack",
        "Vàng cúp, với rào chắn trường đua",
    ),
    (
        "High contrast black and white, plain background",
        "Đen trắng tương phản cao, nền trơn",
    ),
    ("Background", "Hình nền"),
    (
        "Sky and decorations behind the pages",
        "Bầu trời và trang trí phía sau các trang",
    ),
    (
        "The theme's gradient, clouds, petals, stars or rails. Ink has none.",
        "Dải màu, mây, cánh hoa, sao hoặc rào chắn của chủ đề. Chủ đề Mực không có.",
    ),
    (
        "Chibi Seiun Sky, the app's mascot. She's only drawn where nothing else is, so she never covers your data.",
        "Chibi Seiun Sky, linh vật của app. Cô ấy chỉ được vẽ ở chỗ trống nên không bao giờ che dữ liệu của bạn.",
    ),
    ("Everywhere", "Mọi nơi"),
    (
        "On Home, on empty pages, and small beside every page title",
        "Ở Trang chủ, các trang trống, và nhỏ cạnh tiêu đề mỗi trang",
    ),
    ("Home and empty pages", "Trang chủ và trang trống"),
    (
        "Beside Home's welcome, and on pages with nothing to show yet",
        "Cạnh lời chào ở Trang chủ, và trên các trang chưa có gì để hiển thị",
    ),
    ("Off", "Tắt"),
    ("Nowhere", "Không hiện ở đâu"),
    ("Size", "Kích thước"),
    (
        "Makes everything bigger or smaller, text and all. Ctrl+plus and Ctrl+minus do the same for this session.",
        "Phóng to hoặc thu nhỏ mọi thứ, kể cả chữ. Ctrl+cộng và Ctrl+trừ làm tương tự cho phiên này.",
    ),
    ("Reset to 100%", "Đặt lại 100%"),
    ("Density", "Mật độ"),
    (
        "How much space there is between things.",
        "Khoảng cách giữa các thành phần.",
    ),
    ("Comfortable", "Thoải mái"),
    ("Roomier, easier to read", "Rộng rãi, dễ đọc hơn"),
    ("Compact", "Gọn"),
    (
        "Tighter spacing, more on screen",
        "Khoảng cách hẹp, hiện được nhiều hơn",
    ),
    // Settings → Appearance → Navigation.
    ("Navigation", "Điều hướng"),
    (
        "Choose which tabs appear in the top bar. Home is always shown.",
        "Chọn các tab hiển thị trên thanh menu trên cùng. Trang chủ luôn luôn hiển thị.",
    ),
    ("📚 Collection", "📚 Bộ sưu tập"),
    ("🏇 Career", "🏇 Career"),
    ("💬 Ask", "💬 Hỏi đáp"),
    ("🎵 Music", "🎵 Âm nhạc"),
    (
        "Show or hide the Collection tab",
        "Ẩn hoặc hiện tab Bộ sưu tập",
    ),
    ("Show or hide the Career tab", "Ẩn hoặc hiện tab Career"),
    ("Show or hide the Ask tab", "Ẩn hoặc hiện tab Hỏi đáp"),
    (
        "Show or hide the Music tab (when Game audio is enabled)",
        "Ẩn hoặc hiện tab Âm nhạc (khi tính năng Âm thanh game được bật)",
    ),
    (
        "Home is always shown as a safe default. Hidden tabs can still be accessed via shortcuts.",
        "Trang chủ luôn luôn hiển thị làm mặc định an toàn. Các tab bị ẩn vẫn có thể truy cập qua phím tắt.",
    ),
    ("📌 Pinned favorite tab", "📌 Tab yêu thích đã ghim"),
    (
        "Pinned favorite (Right-click to unpin)",
        "Tab yêu thích đã ghim (Bấm chuột phải để bỏ ghim)",
    ),
    ("Right-click to pin", "Bấm chuột phải để ghim"),
    ("Unpin tab", "Bỏ ghim tab"),
    ("📌 Pin as favorite", "📌 Ghim làm tab yêu thích"),
    ("Open pinned tab on startup", "Mở tab đã ghim khi khởi động"),
    (
        "Start UmaYu on your pinned favorite tab instead of Home",
        "Khởi động UmaYu tại tab yêu thích đã ghim thay vì Home",
    ),
    ("Pinned tab: {0} {1}", "Tab đã ghim: {0} {1}"),
    (
        "Pinned tab: {0} {1} (Currently hidden — Home will open on startup)",
        "Tab đã ghim: {0} {1} (Đang bị ẩn — Home sẽ mở khi khởi động)",
    ),
    (
        "No pinned tab. Right-click any tab in the top bar to pin it.",
        "Chưa ghim tab nào. Bấm chuột phải vào bất kỳ tab nào trên thanh menu để ghim.",
    ),
    ("Unpin", "Bỏ ghim"),
    // Settings → Data and updates.
    ("unknown", "không rõ"),
    ("Champions Meeting schedule", "Lịch Champions Meeting"),
    (
        "Official dates, race conditions and the skills that won't activate come from the game's news on umamusume.com. Umalator (umalator.app) gives the exact course and estimates the events not announced yet.",
        "Ngày chính thức, điều kiện đua và các skill sẽ không kích hoạt lấy từ tin tức của game trên umamusume.com. Umalator (umalator.app) cung cấp chính xác đường đua và ước tính các sự kiện chưa được công bố.",
    ),
    (
        "Fetch the latest schedule when the app starts (at most every 6 hours)",
        "Tải lịch mới nhất khi app khởi động (tối đa 6 giờ một lần)",
    ),
    ("not fetched yet", "chưa tải"),
    ("· fetched {}", "· đã tải {}"),
    ("Checking…", "Đang kiểm tra…"),
    ("🔄 Refresh now", "🔄 Làm mới ngay"),
    (
        "Fetch the news and Umalator's presets again",
        "Tải lại tin tức và preset của Umalator",
    ),
    ("Last checked {}.", "Kiểm tra lần cuối {}."),
    ("Game data", "Dữ liệu game"),
    (
        "Every uma, support card, skill, race and event on the Global server.",
        "Mọi uma, support card, skill, race và event trên server Global.",
    ),
    (
        "Built {} · {} umas · {} support cards · {} skills · {} races",
        "Tạo ngày {} · {} uma · {} support card · {} skill · {} race",
    ),
    (
        "The race simulator's skills and courses, and its Champions Meeting presets (Moomoolator, umalator.app).",
        "Skill và đường đua của trình mô phỏng đua, cùng preset Champions Meeting của nó (Moomoolator, umalator.app).",
    ),
    (
        "Built {} · {} courses · {} event presets",
        "Tạo ngày {} · {} đường đua · {} preset sự kiện",
    ),
    (
        "Read {} · community opinion, not game data",
        "Đọc ngày {} · ý kiến cộng đồng, không phải dữ liệu game",
    ),
    (
        "Not downloaded (knowledge/data/pvp_meta.json is missing).",
        "Chưa tải (thiếu knowledge/data/pvp_meta.json).",
    ),
    ("Skill tier list", "Bảng tier skill"),
    (
        "The community skills spreadsheet's Team Trials and Champions Meeting tiers, shown on skills and used by the Rating auto-pick.",
        "Tier Team Trials và Champions Meeting từ bảng tính skill của cộng đồng, hiển thị trên skill và dùng cho auto-pick theo Rating.",
    ),
    (
        "Sheet updated {} · {} skills · community opinion, not game data",
        "Sheet cập nhật {} · {} skill · ý kiến cộng đồng, không phải dữ liệu game",
    ),
    (
        "Not downloaded (knowledge/data/skill_tiers.json is missing).",
        "Chưa tải (thiếu knowledge/data/skill_tiers.json).",
    ),
    ("Refreshing it", "Làm mới dữ liệu"),
    (
        "Install Python 3, then run one of these in PowerShell from the UmaYu folder, and press 🔄 Reload (F5). The meta sheet changes most often.",
        "Cài Python 3, rồi chạy một trong các lệnh này trong PowerShell từ thư mục UmaYu, sau đó nhấn 🔄 Tải lại (F5). Meta sheet thay đổi thường xuyên nhất.",
    ),
    ("📋 Copy", "📋 Sao chép"),
    (
        "Copy the command, to paste in PowerShell in the UmaYu folder",
        "Sao chép lệnh để dán vào PowerShell trong thư mục UmaYu",
    ),
    ("Advanced", "Nâng cao"),
    (
        "📝 Browse notes and data files",
        "📝 Xem ghi chú và file dữ liệu",
    ),
    (
        "Read the markdown files in this folder: insights, plans, results, your roster files and the knowledge notes",
        "Đọc các file markdown trong thư mục này: insights, kế hoạch, kết quả, file roster của bạn và các ghi chú kiến thức",
    ),
    ("📂 Open the game data folder", "📂 Mở thư mục dữ liệu game"),
    // Settings → Game audio.
    (
        "Connect to local Uma Musume game installation files to enable Audio Player, Jukebox, and Live Music.",
        "Kết nối với file cài đặt Uma Musume trên máy để sử dụng Trình phát nhạc, Jukebox và Live Music.",
    ),
    ("Enable", "Bật tính năng"),
    (
        "Enable Game Audio features using local game assets.",
        "Bật tính năng Âm thanh game bằng tài nguyên game cục bộ.",
    ),
    ("Enable Game Audio plug-in", "Bật plug-in Âm thanh game"),
    (
        "Game audio is currently disabled. Enable it above to configure and load game tracks.",
        "Âm thanh game hiện đang tắt. Bật ở phía trên để cấu hình và tải các bài nhạc.",
    ),
    ("Output device", "Thiết bị phát âm thanh"),
    (
        "Choose default audio playback output device for Music.",
        "Chọn thiết bị phát âm thanh đầu ra mặc định cho Nhạc.",
    ),
    ("System default (auto)", "Mặc định hệ thống (tự động)"),
    ("System default", "Mặc định hệ thống"),
    ("Active: {}", "Đang dùng: {}"),
    ("🔄 Rescan", "🔄 Quét lại"),
    (
        "Rescan active audio output devices (e.g. newly plugged headphones)",
        "Quét lại các thiết bị âm thanh đang hoạt động (ví dụ: vừa cắm tai nghe)",
    ),
    (
        "Music plays through the selected device instead of the Windows default. Headphones must be connected and active to appear in the list.",
        "Nhạc phát qua thiết bị được chọn, không theo thiết bị mặc định của Windows. Tai nghe phải đang được cắm/kết nối để xuất hiện trong danh sách.",
    ),
    ("Default volume", "Âm lượng mặc định"),
    (
        "Default music playback volume applied on startup.",
        "Âm lượng nhạc mặc định áp dụng khi khởi động.",
    ),
    ("Default vocal volume", "Âm lượng vocal mặc định"),
    (
        "Default vocal volume for Music Live performers applied on startup.",
        "Âm lượng vocal mặc định cho các ca sĩ trong Music Live khi khởi động.",
    ),
    (
        "⚠ Volumes above 200% may cause excessive loudness and risk hearing damage.",
        "⚠ Mức âm lượng trên 200% có thể gây âm lượng nguy hiểm và tổn hại thính giác.",
    ),
    ("High volume warning", "Cảnh báo âm lượng lớn"),
    (
        "Setting volume above 200% may cause excessive loudness, audio clipping, and risk hearing damage. Do you want to continue?",
        "Thiết lập âm lượng trên 200% có thể gây âm lượng quá mức, méo tiếng và nguy cơ tổn hại thính giác. Bạn có muốn tiếp tục?",
    ),
    ("Continue", "Tiếp tục"),
    (
        "Audio output switched to {}",
        "Đã chuyển thiết bị phát sang {}",
    ),
    ("Device not found: {}", "Không tìm thấy thiết bị: {}"),
    (
        "Default audio device not available",
        "Không có thiết bị âm thanh mặc định",
    ),
    ("Game directory", "Thư mục game"),
    (
        "Path to the Persistent directory containing dat, meta, and master folders.",
        "Đường dẫn tới thư mục Persistent chứa các thư mục dat, meta và master.",
    ),
    ("e.g. C:\\...\\Persistent", "vd. C:\\...\\Persistent"),
    ("🔍 Auto-detect", "🔍 Dò tự động"),
    (
        "Scan for Global, Steam JP, and DMM JP installations",
        "Quét tìm các bản cài đặt Global, Steam JP và DMM JP",
    ),
    (
        "Reload manifest and master databases from disk",
        "Tải lại manifest và database master từ đĩa",
    ),
    ("Detected games", "Bản cài phát hiện được"),
    (
        "Found game installations on this computer.",
        "Các bản cài đặt game tìm thấy trên máy tính này.",
    ),
    ("Valid", "Hợp lệ"),
    ("Incomplete", "Chưa đủ file"),
    (
        "Current status of the game data reader.",
        "Trạng thái hiện tại của trình đọc dữ liệu game.",
    ),
    ("Disabled", "Đã tắt"),
    ("No game directory selected.", "Chưa chọn thư mục game."),
    ("Idle. Ready to load.", "Sẵn sàng tải."),
    ("Scanning for installations…", "Đang quét tìm bản cài đặt…"),
    (
        "Loading manifest and master databases…",
        "Đang nạp manifest và database master…",
    ),
    ("Loaded successfully.", "Đã tải thành công."),
    (
        "Manifest: {} audio entries · Master: {} live songs, {} jukebox tracks",
        "Manifest: {} mục audio · Master: {} bài live, {} bài jukebox",
    ),
    ("Advanced: Key", "Nâng cao: Khóa giải mã"),
    (
        "Custom hex encryption key for meta database (optional).",
        "Khóa giải mã hex tùy chỉnh cho database meta (tùy chọn).",
    ),
    (
        "Leave blank to use standard game keys",
        "Để trống để dùng khóa mặc định của game",
    ),
    // Settings → AI assistant.
    ("Answered by", "Người trả lời"),
    (
        "Who answers your questions on the Ask tab.",
        "Ai trả lời câu hỏi của bạn ở tab Hỏi đáp.",
    ),
    ("🆓 OpenCode · free models", "🆓 OpenCode · model miễn phí"),
    (
        "Free models through OpenCode. No account or key; weaker than Claude.",
        "Model miễn phí qua OpenCode. Không cần tài khoản hay key; yếu hơn Claude.",
    ),
    ("🔑 Claude API · your key", "🔑 Claude API · key của bạn"),
    (
        "Claude with your own Anthropic API key; use is billed to that key.",
        "Claude với API key Anthropic của riêng bạn; chi phí tính vào key đó.",
    ),
    (
        "A question is being answered; switch when it's done.",
        "Đang trả lời một câu hỏi; hãy đổi khi xong.",
    ),
    // Settings → About.
    (
        "Your Uma Musume: Pretty Derby (Global) companion. Plan careers on the community meta, race them in a real simulator, and keep your whole collection at hand.",
        "Người bạn đồng hành Uma Musume: Pretty Derby (Global) của bạn. Lên kế hoạch career theo meta cộng đồng, cho chạy đua trong trình mô phỏng thật, và luôn có cả bộ sưu tập trong tầm tay.",
    ),
    ("Releases and downloads", "Bản phát hành và tải về"),
    (
        "Report a bug or suggest an idea",
        "Báo lỗi hoặc góp ý tưởng",
    ),
    ("Credits", "Ghi công"),
    ("Original project", "Dự án gốc"),
    (
        "UmaYu is a fork of brickc4's umaLord, the companion app this project builds on",
        "UmaYu là bản fork của umaLord (brickc4), ứng dụng đồng hành mà dự án này kế thừa",
    ),
    ("Race simulator", "Trình mô phỏng đua"),
    (
        "third-party crates cridecoder (CRIWARE ACB/AWB/HCA decoding), cpal (WASAPI audio output), rusqlite (SQLite manifest reading), lz4_flex (UnityFS LZ4 decompression)",
        "các crate bên thứ ba cridecoder (giải mã CRIWARE ACB/AWB/HCA), cpal (xuất âm thanh WASAPI), rusqlite (đọc manifest SQLite), lz4_flex (giải nén UnityFS LZ4)",
    ),
    ("Game audio reference", "Nguồn tham khảo Game audio"),
    (
        "UmamusumeExplorer by MarshmallowAndroid: inspiration and reference for the game audio concept",
        "UmamusumeExplorer của MarshmallowAndroid: nguồn cảm hứng và tham khảo ý tưởng tính năng âm thanh game",
    ),
    ("Meta picks", "Lựa chọn meta"),
    ("Skill tiers", "Tier skill"),
    ("Glossary and tips", "Thuật ngữ và mẹo"),
    ("Chibi art", "Hình chibi"),
    (
        "uma.guide: umas, cards, skills, races and events",
        "uma.guide: uma, card, skill, race và event",
    ),
    (
        "a port of Moomoolator (TheCing/uma-tools), which builds on alpha123's Umalator and kachi-dev's VFalator",
        "bản port của Moomoolator (TheCing/uma-tools), dựa trên Umalator của alpha123 và VFalator của kachi-dev",
    ),
    (
        "the community \"PvP Deck & Uma Recommendation\" sheet by Lightning, Daptrius and Henry",
        "sheet cộng đồng \"PvP Deck & Uma Recommendation\" của Lightning, Daptrius và Henry",
    ),
    (
        "the community \"Uma Musume Skills Spreadsheet\" tier list by Dylan, formatted by sayaduck",
        "bảng tier \"Uma Musume Skills Spreadsheet\" của cộng đồng do Dylan lập, sayaduck trình bày",
    ),
    (
        "GameTora, uma.guide and other community guides, each cited in knowledge/mechanics.md",
        "GameTora, uma.guide và các hướng dẫn cộng đồng khác, mỗi nguồn đều được trích dẫn trong knowledge/mechanics.md",
    ),
    ("License", "Giấy phép"),
    (
        "GPL-3.0-or-later: the race simulator ports Moomoolator, which is GPL-3.0-or-later. The source is on GitHub and in the app folder.",
        "GPL-3.0-or-later: trình mô phỏng đua là bản port của Moomoolator, vốn theo GPL-3.0-or-later. Mã nguồn có trên GitHub và trong thư mục app.",
    ),
    (
        "Uma Musume: Pretty Derby is © Cygames, Inc. UmaYu is an unofficial fan project, not affiliated with or endorsed by Cygames.",
        "Uma Musume: Pretty Derby là © Cygames, Inc. UmaYu là dự án fan không chính thức, không liên kết hay được Cygames bảo trợ.",
    ),
    ("Keyboard shortcuts", "Phím tắt"),
    // Keyboard shortcuts.
    (
        "Switch tabs: Home, Collection, Career, Ask",
        "Chuyển tab: Trang chủ, Bộ sưu tập, Career, Hỏi đáp",
    ),
    ("Open Settings", "Mở Cài đặt"),
    (
        "Jump to the page's search box",
        "Nhảy tới ô tìm kiếm của trang",
    ),
    (
        "Save the plan (on Career → Plan)",
        "Lưu kế hoạch (ở Career → Kế hoạch)",
    ),
    (
        "Reload your roster, game data and plans from disk",
        "Tải lại roster, dữ liệu game và kế hoạch từ ổ đĩa",
    ),
    (
        "Close Help, a dialog, or the details panel",
        "Đóng Trợ giúp, hộp thoại hoặc bảng chi tiết",
    ),
    ("Open this Help", "Mở Trợ giúp này"),
    // The tabs.
    ("Home", "Trang chủ"),
    (
        "Your stable at a glance, the next Champions Meeting, and shortcuts to common jobs.",
        "Tổng quan đội uma của bạn, Champions Meeting kế tiếp và lối tắt cho các việc thường làm.",
    ),
    ("Collection", "Bộ sưu tập"),
    (
        "Everything you own: umas and support cards, plus every skill in the game and where you can get it.",
        "Mọi thứ bạn sở hữu: uma và support card, cùng mọi skill trong game và nơi có thể lấy được.",
    ),
    (
        "Plan a career, race the build against others in the simulator, and log what really happened.",
        "Lên kế hoạch career, cho build đua với các build khác trong trình mô phỏng, và ghi lại kết quả thực tế.",
    ),
    ("Ask", "Hỏi đáp"),
    (
        "Ask questions about your account in plain English and get answers from your own data.",
        "Đặt câu hỏi về tài khoản bằng ngôn ngữ tự nhiên và nhận câu trả lời từ chính dữ liệu của bạn.",
    ),
    ("Notes", "Ghi chú"),
    (
        "Read the notes, plans and insights saved in this folder.",
        "Đọc ghi chú, kế hoạch và insights đã lưu trong thư mục này.",
    ),
    ("Plan", "Kế hoạch"),
    (
        "Plan a career step by step: goal, trainee, deck of support cards, and skills to buy.",
        "Lên kế hoạch career từng bước: mục tiêu, trainee, deck support card và skill cần mua.",
    ),
    ("Race sim", "Mô phỏng đua"),
    (
        "Race two builds against each other many times to see which one wins, and by how much.",
        "Cho hai build đua với nhau nhiều lần để xem bên nào thắng và thắng bao nhiêu.",
    ),
    ("Results", "Kết quả"),
    (
        "Write down what really happened in careers and events, so plans can be checked against it.",
        "Ghi lại những gì thực sự xảy ra trong career và sự kiện, để đối chiếu với kế hoạch.",
    ),
    // Top bar, toasts and status bar.
    (
        "Settings (Ctrl+,): your profile, language, look and theme, game data, the AI assistant",
        "Cài đặt (Ctrl+,): hồ sơ, ngôn ngữ, giao diện và chủ đề, dữ liệu game, trợ lý AI",
    ),
    (
        "Help (?): what each tab does, how to plan a career, and the game's words",
        "Trợ giúp (?): công dụng từng tab, cách lên kế hoạch career và thuật ngữ game",
    ),
    (
        "Reload (F5): re-read your roster, game data and plans from disk",
        "Tải lại (F5): đọc lại roster, dữ liệu game và kế hoạch từ ổ đĩa",
    ),
    (
        "Reloaded your roster, game data and plans.",
        "Đã tải lại roster, dữ liệu game và kế hoạch.",
    ),
    ("Couldn't open {}: {}", "Không mở được {}: {}"),
    (
        "Ctrl+S saves the plan on Career → Plan.",
        "Ctrl+S lưu kế hoạch ở Career → Kế hoạch.",
    ),
    (
        "Whose account this is. ⚙ Settings → Profile renames it or switches accounts.",
        "Tài khoản này của ai. ⚙ Cài đặt → Hồ sơ để đổi tên hoặc chuyển tài khoản.",
    ),
    ("📁 Data folder", "📁 Thư mục dữ liệu"),
    ("📁 Your data folder", "📁 Thư mục dữ liệu của bạn"),
    (
        "⚠ Using a built-in snapshot: the UmaYu folder wasn't found",
        "⚠ Đang dùng bản dữ liệu dựng sẵn: không tìm thấy thư mục UmaYu",
    ),
    ("Roster last updated {}", "Roster cập nhật lần cuối {}"),
    (
        "Your roster files were last updated {}.",
        "File roster của bạn được cập nhật lần cuối {}.",
    ),
    (
        "⚠ Game data missing: run tools/update_knowledge.py",
        "⚠ Thiếu dữ liệu game: hãy chạy tools/update_knowledge.py",
    ),
    (
        "Game data from uma.guide, {}",
        "Dữ liệu game từ uma.guide, {}",
    ),
    ("Game data from uma.guide", "Dữ liệu game từ uma.guide"),
    (
        "Game data from uma.guide{}. Global server: {} umas · {} support cards · {} skills",
        "Dữ liệu game từ uma.guide{}. Server Global: {} uma · {} support card · {} skill",
    ),
    ("⚠ {} unread lines", "⚠ {} dòng không đọc được"),
    (
        "⚠ {} lines in your roster files couldn't be read",
        "⚠ {} dòng trong file roster không đọc được",
    ),
    (
        "{} lines in your roster files couldn't be read:\n{}",
        "{} dòng trong file roster không đọc được:\n{}",
    ),
    // Help.
    ("❓ Help", "❓ Trợ giúp"),
    ("What each tab is for", "Công dụng của từng tab"),
    (
        "Overview of UmaYu's main features and navigation.",
        "Tổng quan về các chức năng chính và cách điều hướng trong UmaYu.",
    ),
    ("Go there", "Đi tới đó"),
    ("All", "Tất cả"),
    ("Pages & tabs", "Trang & tab"),
    ("Career workflow", "Quy trình career"),
    ("Shortcuts", "Phím tắt"),
    ("Glossary", "Từ điển thuật ngữ"),
    ("Music", "Âm nhạc"),
    ("(Beta)", "(Beta)"),
    ("Music (Beta)", "Âm nhạc (Beta)"),
    (
        "Game audio library: browse sound files, jukebox songs, and live music.",
        "Thư viện âm thanh game: duyệt file âm thanh, bài hát jukebox và sân khấu live.",
    ),
    ("Umas", "Uma"),
    (
        "Owned umas, outfits, star ranks and awakened potential levels.",
        "Các uma sở hữu, outfit, hạng sao và cấp potential thức tỉnh.",
    ),
    ("Support cards", "Support card"),
    (
        "Owned support cards, limit break levels and training effects.",
        "Các support card sở hữu, cấp limit break và hiệu ứng huấn luyện.",
    ),
    ("Skills", "Skill"),
    (
        "Complete skill catalog, SP costs, and acquisition sources.",
        "Toàn bộ danh mục skill trong game, giá SP và nguồn sở hữu.",
    ),
    ("Files", "File"),
    (
        "Browse and inspect local CRIWARE audio archives (ACB/AWB), play wave cues, and export WAV.",
        "Duyệt và kiểm tra file âm thanh CRIWARE cục bộ (ACB/AWB), phát wave cue và xuất WAV.",
    ),
    ("Jukebox", "Jukebox"),
    (
        "In-game jukebox catalog with short preview and full game-size playback.",
        "Danh mục jukebox trong game với bản nghe thử ngắn và bản game-size hoàn chỉnh.",
    ),
    ("Live", "Live"),
    (
        "Multi-track live stage player with synchronized lyrics and performer lineup controls.",
        "Trình phát sân khấu live đa track kèm lời bài hát đồng bộ và điều khiển đội hình biểu diễn.",
    ),
    ("📝 Notes and files", "📝 Ghi chú và file"),
    (
        "Read the markdown files in this folder: insights, plans, results and your roster files. It's in ⚙ Settings → Data and updates.",
        "Đọc các file markdown trong thư mục này: insights, kế hoạch, kết quả và file roster của bạn. Nằm ở ⚙ Cài đặt → Dữ liệu và cập nhật.",
    ),
    (
        "Planning a career, start to finish",
        "Lên kế hoạch career từ đầu đến cuối",
    ),
    (
        "Recommended step-by-step workflow from goal setting to result logging.",
        "Quy trình từng bước đề xuất từ khi chọn mục tiêu đến lúc ghi nhận kết quả.",
    ),
    (
        "Open 🏇 Career → 📋 Plan and pick a goal. For a Champions Meeting, click the upcoming one and the course fills in, and the 📈 Meta card shows what the community's PvP sheet recommends for it.",
        "Mở 🏇 Career → 📋 Kế hoạch và chọn mục tiêu. Với Champions Meeting, bấm vào kỳ sắp tới để tự điền đường đua, và thẻ 📈 Meta sẽ hiện những gì sheet PvP của cộng đồng đề xuất.",
    ),
    (
        "Pick a trainee. The ✨ suggestions are the sheet's picks you own (★), then your umas with the best aptitudes for that course.",
        "Chọn trainee. Các gợi ý ✨ là lựa chọn của sheet mà bạn sở hữu (★), sau đó là các uma có aptitude tốt nhất cho đường đua đó.",
    ),
    (
        "Press ✨ Recommend whole deck, or click each slot to choose cards yourself. The sheet's cards come first (★); hover a card to see why it was picked.",
        "Nhấn ✨ Gợi ý cả deck, hoặc bấm từng ô để tự chọn card. Card của sheet được ưu tiên (★); rê chuột lên card để xem lý do được chọn.",
    ),
    (
        "Under Skills to buy, press ✨ Auto-pick. It buys the sheet's skills first, then the ones that gain the most on the course, within your skill points.",
        "Ở mục Skill cần mua, nhấn ✨ Auto-pick. Nó mua skill của sheet trước, rồi đến các skill có lợi nhất trên đường đua, trong giới hạn skill point của bạn.",
    ),
    (
        "Press 💾 Save plan. ▶ Simulate races the plan on the 🏁 Race sim page.",
        "Nhấn 💾 Lưu kế hoạch. ▶ Mô phỏng sẽ cho kế hoạch chạy đua ở trang 🏁 Mô phỏng đua.",
    ),
    (
        "After the career, log what happened on the 🏆 Results page.",
        "Sau career, ghi lại kết quả ở trang 🏆 Kết quả.",
    ),
    (
        "Quick actions and navigation keys available anywhere in the app.",
        "Các phím thao tác nhanh và điều hướng dùng được ở mọi nơi trong app.",
    ),
    (
        "Switch tabs: Home, Collection, Career, Ask, Music",
        "Chuyển tab: Trang chủ, Bộ sưu tập, Career, Hỏi đáp, Âm nhạc",
    ),
    (
        "Switch to Music tab (when Game Audio is enabled)",
        "Chuyển sang tab Âm nhạc (khi bật Âm thanh game)",
    ),
    ("Words you'll see", "Thuật ngữ bạn sẽ gặp"),
    (
        "General game terms, in short, from knowledge/mechanics.md. Hover a term to see where it comes from.",
        "Thuật ngữ game chung (giữ nguyên tiếng Anh), tóm tắt từ knowledge/mechanics.md. Rê chuột lên thuật ngữ để xem nguồn.",
    ),
    ("Search terms…", "Tìm thuật ngữ…"),
    (
        "No terms match your search.",
        "Không tìm thấy thuật ngữ phù hợp.",
    ),
    ("Clear search", "Xóa tìm kiếm"),
    ("General knowledge of the game", "Kiến thức chung về game"),
    ("Source: {}", "Nguồn: {}"),
    // Help: Dynamic glossary entries from mechanics.md (faithful to source).
    (
        "A horse girl. You train one in each career; that uma is the trainee.",
        "Một cô gái ngựa. Mỗi career bạn huấn luyện một uma; uma đó là trainee.",
    ),
    (
        "A version of an uma, like [Reeling in the Big One]. Each outfit has its own unique skill and aptitudes, and is trained separately.",
        "Một phiên bản của uma, như [Reeling in the Big One]. Mỗi outfit có unique skill và aptitude riêng, và được huấn luyện độc lập.",
    ),
    (
        "An uma's star rank, 1 to 5. Raising it makes her stronger, including her unique skill. Only 3★ umas pass their unique on as a green spark.",
        "Hạng sao của uma, từ 1 đến 5. Nâng sao giúp uma mạnh hơn, kể cả unique skill. Chỉ uma 3★ trở lên mới truyền unique skill dưới dạng green spark.",
    ),
    (
        "Unlocks an uma's own skills in steps. At P5 all of them are available to buy.",
        "Mở khóa dần các skill riêng của uma theo từng mốc. Ở P5 tất cả đều có thể mua được.",
    ),
    (
        "A letter grade, S down to G, for a surface, a distance and a running style. A or better is what you want; lower grades slow her down, and the style grade also scales her Wit.",
        "Điểm chữ từ S xuống G cho cự ly, mặt sân và running style. Nên đạt từ A trở lên; hạng thấp hơn làm uma chạy chậm đi, và hạng style còn làm giảm chỉ số Wit hiệu dụng.",
    ),
    (
        "Front Runner leads from the start. Pace Chaser runs just behind the leaders. Late Surger waits mid-pack and moves up late. End Closer stays at the back and sprints at the end.",
        "Front Runner dẫn đầu từ lúc xuất phát. Pace Chaser chạy ngay sau nhóm dẫn đầu. Late Surger chờ ở giữa đoàn rồi vươn lên muộn. End Closer ở cuối đoàn và bứt tốc ở đoạn cuối.",
    ),
    (
        "Speed, Stamina, Power, Guts and Wit. Past 1200 a stat counts half in races.",
        "Speed, Stamina, Power, Guts và Wit. Trong cuộc đua, chỉ số vượt quá 1200 chỉ tính một nửa giá trị.",
    ),
    (
        "A trainee's bonus to training gains in some stats, like Wit +20%.",
        "Thưởng thêm của trainee cho lượng tăng khi huấn luyện một số chỉ số, như Wit +20%.",
    ),
    (
        "Up to six cards go with you into a career: five of yours and one borrowed. They boost training and teach skills.",
        "Tối đa sáu card đi cùng bạn vào career: năm card của bạn và một card mượn. Chúng tăng hiệu quả huấn luyện và dạy skill.",
    ),
    (
        "Card rarity, best first. A maxed SR usually beats an SSR at LB0.",
        "Độ hiếm của card, tốt nhất đứng đầu. Một card SR max limit break thường mạnh hơn một card SSR ở LB0.",
    ),
    (
        "Merging copies of a card raises its level cap. LB0 is one copy; LB4, also called MLB, is fully limit broken.",
        "Gộp các bản sao của một card để nâng giới hạn cấp. LB0 là một bản; LB4, còn gọi là MLB, là limit break tối đa.",
    ),
    (
        "Max limit break (LB4): the card at full strength.",
        "Max limit break (LB4): card ở sức mạnh tối đa.",
    ),
    (
        "A card borrowed from another player for one career. It can be any card, at their LB.",
        "Card mượn từ người chơi khác cho một career. Có thể là bất kỳ card nào, ở mức LB của họ.",
    ),
    (
        "A card or event offering a skill at a discount: 10/20/30/35/40% off at levels 1–5.",
        "Card hoặc event giảm giá một skill: giảm 10/20/30/35/40% ở cấp hint 1–5.",
    ),
    (
        "Earned during a career and spent on skills at the end.",
        "Kiếm được trong career và dùng để mua skill ở cuối.",
    ),
    (
        "Ordinary and stronger skills. A gold skill needs its white version first.",
        "Skill thường và skill mạnh hơn. Skill gold thường cần có bản white trước.",
    ),
    (
        "A passive skill that adds stats when its condition fits (ground, weather, season, racecourse). It needs no Wit check and can push a stat past 1200.",
        "Skill bị động tăng chỉ số khi khớp điều kiện (mặt sân, thời tiết, mùa, trường đua). Không cần xét Wit check và có thể đẩy chỉ số vượt mốc 1200.",
    ),
    (
        "Each outfit's own skill. As a parent, an uma can pass a weaker copy on.",
        "Skill riêng của mỗi outfit. Khi làm parent, uma có thể truyền lại một bản yếu hơn.",
    ),
    (
        "A stronger version of a skill some umas can unlock during a career by meeting its conditions.",
        "Phiên bản mạnh hơn của một skill mà một số uma có thể mở khóa trong career khi đạt đủ điều kiện.",
    ),
    (
        "The roll every non-green skill makes to fire: max(20, 100 − 9000 / Wit) %.",
        "Xác suất kích hoạt của mọi skill không phải green skill: max(20, 100 − 9000 / Wit) %.",
    ),
    (
        "How long a skill lasts in a 1000 m race; it scales with the distance (2.4 s becomes 5.76 s at 2400 m).",
        "Thời gian hiệu lực gốc của skill ở cự ly 1000 m; thời lượng này tăng theo cự ly đua (2.4 giây thành 5.76 giây ở cự ly 2400 m).",
    ),
    (
        "How soon a skill can fire again: usually never in the same race; some can, after 30 s × distance / 1000.",
        "Thời gian hồi để skill có thể kích hoạt lại: đa phần không thể kích hoạt lần hai trong cùng cuộc đua; một số skill có thể sau 30 s × cự ly / 1000.",
    ),
    (
        "Early race (first sixth), mid race (to two thirds), late race and last spurt (last sixth). Skills often fire only in one.",
        "Các giai đoạn cuộc đua: early race (1/6 đầu), mid race (đến 2/3), late race và last spurt (1/6 cuối). Skill thường chỉ kích hoạt trong một giai đoạn nhất định.",
    ),
    (
        "Where the last spurt starts, at two thirds of the race: acceleration skills that fire here win races.",
        "Khu vực bắt đầu last spurt, ở mốc 2/3 cuộc đua: các skill gia tốc kích hoạt tại đây quyết định chiến thắng.",
    ),
    (
        "The final push from two thirds of the race, at top speed if she has the HP for it.",
        "Giai đoạn tăng tốc bứt phá từ mốc 2/3 cuộc đua, chạy ở tốc độ tối đa nếu uma còn đủ HP.",
    ),
    (
        "Running the whole last spurt at top speed. It needs enough HP, which comes from Stamina.",
        "Chạy trọn vẹn toàn bộ giai đoạn last spurt ở tốc độ tối đa. Cần đủ HP, đến từ Stamina.",
    ),
    (
        "A speed skill that fires just before the spurt and lasts into it, so the spurt starts faster.",
        "Skill tốc độ kích hoạt ngay trước vạch spurt và kéo dài sang spurt, giúp khởi đầu giai đoạn spurt ở tốc độ cao hơn.",
    ),
    (
        "An acceleration skill that fires at a random point in the late race, so it isn't always useful.",
        "Skill gia tốc kích hoạt ở một điểm ngẫu nhiên trong late race, do đó không phải lúc nào cũng mang lại hiệu quả.",
    ),
    (
        "How umas hold their place in the early and mid race, by switching between speed modes.",
        "Cơ chế duy trì vị trí của các uma trong early race và mid race thông qua chuyển đổi các chế độ tốc độ.",
    ),
    (
        "A position-keep mode that slows an uma that is too close to the leaders.",
        "Chế độ duy trì vị trí làm chậm uma khi chạy quá sát nhóm dẫn đầu.",
    ),
    (
        "When an uma gets over-eager in the early or mid race: she burns HP 1.6 times as fast for up to 12 s. Higher Wit makes it rarer.",
        "Khi uma quá hăng ở early race hoặc mid race: tiêu hao HP nhanh gấp 1.6 lần trong tối đa 12 giây. Chỉ số Wit cao hơn sẽ giúp giảm tỷ lệ bị rushed.",
    ),
    (
        "Front Runners side by side early in the race fight for the lead: faster, but they burn HP.",
        "Các Front Runner chạy cạnh nhau ở đầu cuộc đua tranh giành vị trí dẫn đầu: chạy nhanh hơn nhưng tiêu hao HP nhanh hơn.",
    ),
    (
        "Umas side by side on the final straight push each other: more speed, from Guts.",
        "Các uma chạy sát cạnh nhau trên đoạn thẳng cuối thúc đẩy lẫn nhau: tăng thêm tốc độ dựa trên chỉ số Guts.",
    ),
    (
        "A start delay of 0.066 s or more: the uma loses the start's acceleration. Wit doesn't change it.",
        "Xuất phát trễ từ 0.066 giây trở lên: uma mất đi lượng gia tốc ban đầu khi mở cổng. Chỉ số Wit không làm thay đổi điều này.",
    ),
    (
        "A start delay under 0.02 s; worth 1000 points in Team Trials.",
        "Xuất phát cực nhanh với độ trễ dưới 0.02 giây; được thưởng 1000 điểm trong Team Trials.",
    ),
    (
        "How far ahead one uma finishes. One length is 2.5 m.",
        "Khoảng cách một uma về đích trước. Một length là 2.5 m.",
    ),
    (
        "The ground (Firm to Heavy), weather and season of a race. Some skills only work in some conditions.",
        "Mặt sân (Firm đến Heavy), thời tiết và mùa của cuộc đua. Một số skill chỉ hoạt động trong một số điều kiện.",
    ),
    (
        "What a parent passes to the next trainee: blue for stats, pink for aptitudes, green for her unique skill, white for skills and races.",
        "Thứ parent truyền cho trainee kế tiếp: xanh dương cho chỉ số, hồng cho aptitude, xanh lá cho unique skill, trắng cho skill và race.",
    ),
    (
        "A saved, finished uma used as a legacy for a new career. Two parents give their sparks.",
        "Một uma đã hoàn thành và được lưu, dùng làm di sản cho career mới. Hai parent truyền spark của mình.",
    ),
    (
        "The events at the start of April in the Classic and Senior years when parents' sparks can proc.",
        "Sự kiện diễn ra vào đầu tháng 4 năm Classic và Senior, thời điểm các spark của parent có thể kích hoạt.",
    ),
    (
        "How well a parent and her own parents match. It multiplies every spark's chance: × (1 + affinity / 100).",
        "Mức độ hòa hợp giữa parent và các thế hệ trước. Chỉ số này nhân trực tiếp vào tỷ lệ kích hoạt spark: × (1 + affinity / 100).",
    ),
    (
        "An uma's rating grade after a career (G up to SS and beyond), from her stats and skills. SS makes better spark stars.",
        "Xếp hạng đánh giá của uma sau career (từ G đến SS và cao hơn), tính từ chỉ số và skill. Xếp hạng SS cho số sao spark cao hơn.",
    ),
    (
        "The career's training mode, like URA Finale or Grand Concert. Each one trains differently.",
        "Chế độ huấn luyện của career, như URA Finale hay Grand Concert. Mỗi scenario huấn luyện khác nhau.",
    ),
    (
        "A character the scenario favours: her card or outfit earns extra bonuses in that scenario.",
        "Nhân vật được scenario ưu ái: card hoặc outfit của nhân vật đó nhận thêm phần thưởng bổ sung trong scenario.",
    ),
    (
        "A career condition that makes every skill 10% of its base price cheaper.",
        "Điều kiện trạng thái trong career giúp giảm 10% giá gốc của mọi skill.",
    ),
    (
        "July and August of the Classic and Senior years, when every training facility is at its best level.",
        "Tháng 7 và tháng 8 năm Classic và Senior, thời điểm mọi cơ sở huấn luyện đều đạt cấp độ tối đa (cấp 5).",
    ),
    (
        "A monthly player-versus-player event on one course with set conditions: 3v3v3 races over two rounds and a final.",
        "Sự kiện PvP hằng tháng trên một đường đua với điều kiện cố định: các cuộc đua 3v3v3 qua hai vòng đấu và một trận chung kết.",
    ),
    (
        "The two leagues of a Champions Meeting: Open for lower-ranked umas, Graded for any.",
        "Hai hạng đấu của Champions Meeting: Open dành cho các uma xếp hạng thấp hơn, Graded dành cho mọi uma.",
    ),
    (
        "Champions Meeting brackets: 3 wins in one entry of Round 1 puts you in Group A for Round 2.",
        "Nhánh đấu Champions Meeting: 3 trận thắng trong một lượt tham gia ở Vòng 1 sẽ đưa bạn vào Nhánh A ở Vòng 2.",
    ),
    (
        "Your main uma for a PvP event; in Team Trials the top slot of each distance, which scores 10% more.",
        "Uma chủ lực của bạn trong sự kiện PvP; trong Team Trials là ô trên cùng của mỗi cự ly, nhận thêm 10% điểm số.",
    ),
    (
        "Weekly races of your team of umas against other players' teams, scored on placing and skills.",
        "Các cuộc đua hằng tuần giữa đội uma của bạn với đội người chơi khác, chấm điểm dựa trên thứ hạng về đích và skill kích hoạt.",
    ),
    (
        "A ranked player-versus-player event: daily 3v3 races with NPCs, for points.",
        "Sự kiện PvP tính điểm xếp hạng: các cuộc đua 3v3 hằng ngày kèm NPC để tích lũy điểm số.",
    ),
    (
        "What most strong players currently run. The app's ★ marks come from a community sheet of PvP decks, umas and skills per Champions Meeting.",
        "Những gì đa số người chơi mạnh đang dùng. Dấu ★ trong app lấy từ sheet cộng đồng về deck, uma và skill PvP cho từng Champions Meeting.",
    ),
    // Help: the built-in fallback glossary (the terms themselves stay English).
    (
        "A version of an uma, like [Reeling in the Big One]. Each outfit has its own unique skill and aptitudes.",
        "Một phiên bản của uma, như [Reeling in the Big One]. Mỗi outfit có unique skill và aptitude riêng.",
    ),
    (
        "An uma's star rank, 1 to 5. You raise it with shards, and it makes her stronger, including her unique skill.",
        "Hạng sao của uma, từ 1 đến 5. Nâng bằng shard, giúp uma mạnh hơn, kể cả unique skill.",
    ),
    (
        "A letter grade, S down to G, for how well an uma runs on a surface (turf, dirt), a distance and a running style. A or better is what you want; lower grades slow her down.",
        "Điểm chữ từ S xuống G, cho biết uma chạy tốt thế nào trên mặt sân (turf, dirt), cự ly và running style. Nên từ A trở lên; hạng thấp hơn làm uma chậm đi.",
    ),
    (
        "Speed, Stamina, Power, Guts and Wit (wisdom). Training raises them during a career.",
        "Speed, Stamina, Power, Guts và Wit (wisdom). Huấn luyện trong career sẽ tăng các chỉ số này.",
    ),
    (
        "Card rarity, best first.",
        "Độ hiếm của card, tốt nhất đứng đầu.",
    ),
    (
        "A card or event offering a skill at a discount. More hints for the same skill mean a bigger discount.",
        "Card hoặc event giảm giá một skill. Càng nhiều hint cho cùng skill thì càng giảm nhiều.",
    ),
    (
        "The career's training mode, like URA Finale or Unity Cup. Each one trains differently.",
        "Chế độ huấn luyện của career, như URA Finale hay Unity Cup. Mỗi scenario huấn luyện khác nhau.",
    ),
    (
        "A monthly player-versus-player event on one course with set conditions.",
        "Sự kiện PvP hằng tháng trên một đường đua với điều kiện cố định.",
    ),
    (
        "Races of your team of umas against other players' teams.",
        "Các cuộc đua giữa đội uma của bạn với đội của người chơi khác.",
    ),
    (
        "A ranked player-versus-player event.",
        "Sự kiện PvP xếp hạng.",
    ),
    (
        "Running the last stretch at top speed. It needs enough HP, which comes from Stamina.",
        "Chạy đoạn cuối ở tốc độ tối đa. Cần đủ HP, đến từ Stamina.",
    ),
    (
        "When an uma gets over-eager mid-race and burns HP. Wit lowers the chance.",
        "Khi uma quá hăng giữa cuộc đua và tiêu hao HP. Wit giúp giảm khả năng này.",
    ),
    (
        "Card rarity, best first.",
        "Độ hiếm của card, tốt nhất đứng đầu.",
    ),
    (
        "Merging copies of a card raises its level cap. LB0 is one copy; LB4, also called MLB, is fully limit broken.",
        "Gộp các bản sao của một card để nâng giới hạn cấp. LB0 là một bản; LB4, còn gọi là MLB, là limit break tối đa.",
    ),
    (
        "Max limit break (LB4): the card at full strength.",
        "Max limit break (LB4): card ở sức mạnh tối đa.",
    ),
    (
        "Higher levels make a card's effects stronger, up to the cap its LB allows.",
        "Cấp càng cao thì hiệu ứng card càng mạnh, tới giới hạn mà LB cho phép.",
    ),
    (
        "A card borrowed from another player for one career. It can be any card, at their LB.",
        "Card mượn từ người chơi khác cho một career. Có thể là bất kỳ card nào, ở LB của họ.",
    ),
    (
        "A card or event offering a skill at a discount. More hints for the same skill mean a bigger discount.",
        "Card hoặc event giảm giá một skill. Càng nhiều hint cho cùng skill thì càng giảm nhiều.",
    ),
    (
        "Earned during a career and spent on skills at the end.",
        "Kiếm được trong career và dùng để mua skill ở cuối.",
    ),
    (
        "Ordinary and stronger skills. A gold skill usually needs its white version first.",
        "Skill thường và skill mạnh hơn. Skill gold thường cần có bản white trước.",
    ),
    (
        "Each outfit's own skill. As a parent, an uma can pass a weaker copy on.",
        "Skill riêng của mỗi outfit. Khi làm parent, uma có thể truyền lại một bản yếu hơn.",
    ),
    (
        "A stronger version of a skill some umas can unlock during a career by meeting its conditions.",
        "Phiên bản mạnh hơn của một skill mà một số uma có thể mở khóa trong career khi đạt điều kiện.",
    ),
    (
        "What a parent passes to the next trainee: blue for stats, pink for aptitudes, green for her unique skill, white for skills and races.",
        "Thứ parent truyền cho trainee kế tiếp: xanh dương cho chỉ số, hồng cho aptitude, xanh lá cho unique skill, trắng cho skill và race.",
    ),
    (
        "A saved, finished uma used as a legacy for a new career. Two parents give their sparks.",
        "Một uma đã hoàn thành và được lưu, dùng làm di sản cho career mới. Hai parent truyền spark của mình.",
    ),
    (
        "The career's training mode, like URA Finale or Unity Cup. Each one trains differently.",
        "Chế độ huấn luyện của career, như URA Finale hay Unity Cup. Mỗi scenario huấn luyện khác nhau.",
    ),
    (
        "A monthly player-versus-player event on one course with set conditions.",
        "Sự kiện PvP hằng tháng trên một đường đua với điều kiện cố định.",
    ),
    (
        "Races of your team of umas against other players' teams.",
        "Các cuộc đua giữa đội uma của bạn với đội của người chơi khác.",
    ),
    (
        "A ranked player-versus-player event.",
        "Sự kiện PvP xếp hạng.",
    ),
    (
        "What most strong players currently run. The app's ★ marks come from a community sheet of PvP decks, umas and skills per Champions Meeting.",
        "Những gì đa số người chơi mạnh đang dùng. Dấu ★ trong app lấy từ sheet cộng đồng về deck, uma và skill PvP cho từng Champions Meeting.",
    ),
    (
        "How far ahead one uma finishes. One length is 2.5 m.",
        "Khoảng cách một uma về đích trước. Một length là 2.5 m.",
    ),
    (
        "Running the last stretch at top speed. It needs enough HP, which comes from Stamina.",
        "Chạy đoạn cuối ở tốc độ tối đa. Cần đủ HP, đến từ Stamina.",
    ),
    (
        "When an uma gets over-eager mid-race and burns HP. Wit lowers the chance.",
        "Khi uma quá hăng giữa cuộc đua và tiêu hao HP. Wit giúp giảm khả năng này.",
    ),
    (
        "The ground (Firm to Heavy), weather and season of a race. Some skills only work in some conditions.",
        "Mặt sân (Firm đến Heavy), thời tiết và mùa của cuộc đua. Một số skill chỉ hoạt động trong một số điều kiện.",
    ),
    // Phase 3: Home tab & shared UI widgets.
    // Home: Widget headers & descriptions.
    ("Continue where you left off", "Tiếp tục nơi bạn dừng lại"),
    (
        "Your latest plans, race and results.",
        "Kế hoạch, cuộc đua và kết quả gần đây nhất.",
    ),
    ("Your stable", "Đội uma của bạn"),
    (
        "How much of the game you own, and who still has potential to unlock.",
        "Bạn sở hữu bao nhiêu phần của game và ai còn potential chưa mở khóa.",
    ),
    ("Support cards by type", "Support card theo loại"),
    (
        "Cards for each stat; brighter segments are stronger, maxed (MLB) SSRs first.",
        "Card cho từng chỉ số; đoạn sáng hơn là mạnh hơn, SSR maxed (MLB) đứng đầu.",
    ),
    ("Who can run where", "Ai có thể chạy ở đâu"),
    (
        "Owned umas with A or better at a distance and a running style. Gaps show at a glance.",
        "Uma sở hữu có hạng A trở lên ở cự ly và running style. Khoảng trống thấy rõ ngay.",
    ),
    ("Uma of the day", "Uma trong ngày"),
    (
        "A different one of yours each day.",
        "Mỗi ngày một uma khác nhau của bạn.",
    ),
    ("Trainer card", "Thẻ trainer"),
    (
        "Badges for what you've done here.",
        "Huy hiệu cho những gì bạn đã làm tại đây.",
    ),
    (
        "Your best cards for the next CM",
        "Card tốt nhất cho CM tới",
    ),
    (
        "Owned cards the community meta sheet backs most for the next CM's top style. Community meta: opinion, not game data.",
        "Card sở hữu được sheet meta cộng đồng đề xuất nhiều nhất cho style hàng đầu của CM tới. Meta cộng đồng: ý kiến, không phải dữ liệu game.",
    ),
    ("Missing pieces", "Mảnh ghép còn thiếu"),
    (
        "Cards in the community meta sheet's decks for the next CM that you don't own yet. Community meta: opinion, not game data.",
        "Card trong deck của sheet meta cộng đồng cho CM tới mà bạn chưa sở hữu. Meta cộng đồng: ý kiến, không phải dữ liệu game.",
    ),
    ("Did you know?", "Bạn có biết?"),
    (
        "A game fact or term, with where it comes from. A new one each day.",
        "Một sự thật hoặc thuật ngữ game cùng nguồn gốc. Mỗi ngày một điều mới.",
    ),
    // Home: Hero header & days.
    ("Good morning", "Chào buổi sáng"),
    ("Good afternoon", "Chào buổi chiều"),
    ("Good evening", "Chào buổi tối"),
    ("Hello, night owl", "Cú đêm chào bạn"),
    ("Sunday", "Chủ nhật"),
    ("Monday", "Thứ hai"),
    ("Tuesday", "Thứ ba"),
    ("Wednesday", "Thứ tư"),
    ("Thursday", "Thứ năm"),
    ("Friday", "Thứ sáu"),
    ("Saturday", "Thứ bảy"),
    ("January", "Tháng 1"),
    ("February", "Tháng 2"),
    ("March", "Tháng 3"),
    ("April", "Tháng 4"),
    ("May", "Tháng 5"),
    ("June", "Tháng 6"),
    ("July", "Tháng 7"),
    ("August", "Tháng 8"),
    ("September", "Tháng 9"),
    ("October", "Tháng 10"),
    ("November", "Tháng 11"),
    ("December", "Tháng 12"),
    ("{0}, {1} {2}", "{0}, {2} {1}"),
    (
        "{0} · {1} umas · {2} support cards",
        "{0} · {1} uma · {2} support card",
    ),
    ("Trainer", "Trainer"),
    ("Plan a career", "Lên kế hoạch career"),
    ("Career → Plan", "Career → Kế hoạch"),
    ("Race two builds", "Đua hai build"),
    (
        "Race two builds against each other",
        "Cho hai build đua với nhau",
    ),
    ("Add pulls", "Thêm lượt quay"),
    (
        "Add new cards, or a card's new limit break",
        "Thêm card mới, hoặc limit break mới của card",
    ),
    ("Look up a skill", "Tra cứu skill"),
    ("Collection → Skills", "Bộ sưu tập → Skill"),
    ("Ask umaLord", "Hỏi umaLord"),
    (
        "Questions about your account in plain English",
        "Đặt câu hỏi về tài khoản bằng ngôn ngữ tự nhiên",
    ),
    (
        "Seiun Sky. ⚙ Settings moves or hides her.",
        "Seiun Sky. ⚙ Cài đặt để chuyển chỗ hoặc ẩn cô ấy.",
    ),
    ("Your collection is empty", "Bộ sưu tập của bạn đang trống"),
    (
        "Load a saved profile, or click the umas and cards you own. Plans and picks need them.",
        "Tải một profile đã lưu hoặc bấm chọn các uma và card bạn sở hữu. Kế hoạch và gợi ý cần chúng.",
    ),
    ("✨ Set up", "✨ Thiết lập"),
    // Home: Champions Meeting section.
    ("Champions Meeting on now", "Champions Meeting đang diễn ra"),
    ("Next Champions Meeting", "Champions Meeting kế tiếp"),
    ("✔ Announced", "✔ Đã công bố"),
    ("≈ Estimated", "≈ Ước tính"),
    (
        "Race your plan's trainee with and without her skills to buy",
        "Cho trainee trong kế hoạch đua thử khi có và không có skill cần mua",
    ),
    ("📋 Continue plan", "📋 Tiếp tục kế hoạch"),
    (
        "Start a new plan with this course and its conditions filled in",
        "Bắt đầu kế hoạch mới với đường đua và điều kiện này được điền sẵn",
    ),
    ("📋 Plan for it", "📋 Lên kế hoạch"),
    ("Announcement", "Thông báo"),
    ("As announced: {}", "Theo thông báo: {}"),
    ("· trained in {}", "· luyện trong {}"),
    ("Won't activate:", "Sẽ không kích hoạt:"),
    (
        "The official announcement turns these skills off for this Champions Meeting. Auto-pick never buys them for it.",
        "Thông báo chính thức tắt các skill này trong Champions Meeting này. Auto-pick sẽ không bao giờ mua chúng.",
    ),
    ("League selection", "Chọn League"),
    ("Round 1", "Vòng 1"),
    ("Ends", "Kết thúc"),
    (
        "Round 1 around {}{}, by Umalator's estimate. The official dates come with the announcement.",
        "Vòng 1 vào khoảng {}{}, theo ước tính của Umalator. Ngày chính thức sẽ có khi có thông báo.",
    ),
    (
        "{}\nShown in your computer's time zone.",
        "{}\nHiển thị theo múi giờ trên máy tính của bạn.",
    ),
    ("Community meta", "Meta cộng đồng"),
    (
        "{}\n{}\nOpinion, not game data. Read {}.",
        "{}\n{}\nÝ kiến, không phải dữ liệu game. Đọc {}.",
    ),
    (
        "The community sheet has nothing for this CM yet.",
        "Sheet cộng đồng chưa có thông tin cho CM này.",
    ),
    ("Recommended stats", "Chỉ số đề xuất"),
    ("Your best {}s for it", "{} tốt nhất của bạn cho kỳ này"),
    (
        "The community sheet's picks you own first (★), then your umas with the best aptitudes for this course and style.",
        "Lựa chọn của sheet cộng đồng mà bạn sở hữu đứng trước (★), sau đó là các uma có aptitude tốt nhất cho đường đua và style này.",
    ),
    (
        "Add umas in Collection to see who fits.",
        "Thêm uma trong Bộ sưu tập để xem ai phù hợp.",
    ),
    (
        "\n⚠ Below B here: she'd be slow without pink sparks from parents.",
        "\n⚠ Dưới hạng B tại đây: cô ấy sẽ chậm nếu không có spark hồng từ parent.",
    ),
    (
        "{} [{}]{}\nClick to plan the CM with her",
        "{} [{}]{}\nBấm để lên kế hoạch CM với cô ấy",
    ),
    ("Your plan", "Kế hoạch của bạn"),
    (
        "No plan for this CM yet. Plan for it fills in the course and conditions.",
        "Chưa có kế hoạch cho CM này. Nút Lên kế hoạch sẽ điền sẵn đường đua và điều kiện.",
    ),
    ("Pick a trainee", "Chọn trainee"),
    ("Trainee: {}", "Trainee: {}"),
    ("Deck: {} of 6 cards", "Deck: {}/6 card"),
    ("Simulated", "Đã mô phỏng"),
    ("Not simulated yet", "Chưa mô phỏng"),
    ("New plan for the {}; {}.", "Kế hoạch mới cho {}; {}."),
    (
        "New plan for the {} ({}): course and conditions filled in. Next, pick a trainee.",
        "Kế hoạch mới cho {} ({}): đã điền đường đua và điều kiện. Tiếp theo, hãy chọn trainee.",
    ),
    // Home: Timeline.
    ("Coming up", "Sắp diễn ra"),
    (
        "The next three weeks from the game's news. Click a bar to read its announcement.",
        "Ba tuần tới từ tin tức của game. Bấm vào một thanh để đọc thông báo.",
    ),
    (
        "Nothing announced for the next three weeks yet.",
        "Chưa có thông báo nào cho ba tuần tới.",
    ),
    (
        "The news hasn't been fetched yet (Settings → Data and updates).",
        "Chưa tải tin tức (Cài đặt → Dữ liệu và cập nhật).",
    ),
    ("Today", "Hôm nay"),
    ("Click to read the announcement.", "Bấm để đọc thông báo."),
    ("Story and events", "Cốt truyện và sự kiện"),
    ("Scouts (banners)", "Tuyển mộ (banner)"),
    ("Campaigns", "Chiến dịch"),
    (
        "\nEstimated by Umalator: not announced yet",
        "\nƯớc tính bởi Umalator: chưa có thông báo chính thức",
    ),
    // Home: Continue.
    (
        "Nothing to pick up yet. Plans you save, races you simulate and results you log show up here.",
        "Chưa có gì để tiếp tục. Kế hoạch bạn lưu, cuộc đua bạn mô phỏng và kết quả bạn ghi lại sẽ xuất hiện ở đây.",
    ),
    (
        "No saved plans yet: Career → Plan makes one.",
        "Chưa có kế hoạch nào được lưu: Career → Kế hoạch để tạo một kế hoạch.",
    ),
    ("saved {}", "đã lưu {}"),
    ("🏁 Last race:", "🏁 Cuộc đua gần nhất:"),
    (
        "{} won by {} L on average over {} runs.",
        "{0} thắng trung bình {1} L qua {2} lượt chạy.",
    ),
    ("Open", "Mở"),
    ("Latest results", "Kết quả mới nhất"),
    // Home: Stable & Cards by Type & Aptitude.
    ("Umas", "Uma"),
    ("SSR cards", "Thẻ SSR"),
    ("SR cards", "Thẻ SR"),
    ("{0} of {1}", "{0} / {1}"),
    (
        "You own {} of the {} {} released on Global ({}%).",
        "Bạn sở hữu {0} trong số {1} {2} đã phát hành trên Global ({3}%).",
    ),
    ("Potential to raise ({})", "Potential cần nâng ({})"),
    (
        "Umas with skills still locked behind potential, highest stars first. Click one to see which.",
        "Uma còn skill bị khóa sau potential, xếp theo sao cao nhất trước. Bấm vào để xem chi tiết.",
    ),
    (
        "Every uma is at P5. Nice!",
        "Mọi uma đều đã đạt P5. Tuyệt vời!",
    ),
    ("+{} more", "+{} nữa"),
    ("No support cards yet.", "Chưa có support card nào."),
    ("SSR, maxed", "SSR, maxed"),
    ("SR, maxed", "SR, maxed"),
    (
        "{}: SSR maxed {} · other SSR {} · SR maxed {} · other SR {}",
        "{}: SSR maxed {} · SSR khác {} · SR maxed {} · SR khác {}",
    ),
    ("nobody with A or better yet", "chưa có ai đạt A trở lên"),
    (
        "No one yet for: {}. Pink sparks from parents can raise an aptitude.",
        "Chưa có ai cho: {}. Spark hồng từ parent có thể nâng aptitude.",
    ),
    // Home: Uma of the Day & Trainer Card.
    (
        "Add umas to meet one each day.",
        "Thêm uma để gặp một bạn mỗi ngày.",
    ),
    ("Another one", "Người khác"),
    ("Best at {}", "Tốt nhất ở {}"),
    ("Unique", "Unique"),
    ("📋 Plan with her", "📋 Lên kế hoạch với cô ấy"),
    ("trainer since {}", "trainer từ {}"),
    ("{0} of {1} badges", "{0}/{1} huy hiệu"),
    ("Earned: {}", "Đã đạt: {}"),
    ("To earn it: {}", "Để đạt được: {}"),
    ("Stable of 50", "Đội 50 uma"),
    ("Own 50 umas", "Sở hữu 50 uma"),
    ("Hundred cards", "Một trăm card"),
    ("Own 100 support cards", "Sở hữu 100 support card"),
    ("Full house", "Bộ sưu tập đủ loại"),
    (
        "A maxed (MLB) SSR of every stat type: Speed, Stamina, Power, Guts and Wit",
        "Một SSR max (MLB) của mỗi loại chỉ số: Speed, Stamina, Power, Guts và Wit",
    ),
    ("Unlocked", "Mở khóa"),
    ("10 umas at Potential 5", "10 uma đạt Potential 5"),
    ("Five stars", "Năm sao"),
    ("A 5★ uma", "Một uma 5★"),
    ("First CM plan", "Kế hoạch CM đầu tiên"),
    (
        "Save a plan for a Champions Meeting",
        "Lưu một kế hoạch cho Champions Meeting",
    ),
    ("First race", "Cuộc đua đầu tiên"),
    (
        "Race a build in the simulator",
        "Cho một build đua trong trình mô phỏng",
    ),
    ("On the record", "Ghi vào kỷ lục"),
    (
        "Log a real result on Career → Results",
        "Ghi lại một kết quả thực tế ở Career → Kết quả",
    ),
    // Home: Showcase & Wishlist.
    (
        "The community sheet has nothing for the next CM yet.",
        "Sheet cộng đồng chưa có thông tin cho CM kế tiếp.",
    ),
    ("For the {} as a {}", "Cho {} ở vị trí {}"),
    (
        "None of the sheet's core cards for it are in your collection yet: see Missing pieces.",
        "Chưa có card cốt lõi nào của sheet trong bộ sưu tập: xem mục Mảnh ghép còn thiếu.",
    ),
    (
        "You own every card in the sheet's decks for the {}.",
        "Bạn sở hữu tất cả card trong các deck của sheet cho {}.",
    ),
    // Shared UI widgets (ui_util.rs).
    ("🔍 Type to search…", "🔍 Nhập để tìm kiếm…"),
    (
        "…more; keep typing to narrow it down",
        "…còn nữa; tiếp tục nhập để thu hẹp kết quả",
    ),
    ("No match.", "Không tìm thấy."),
    ("Clear", "Xóa"),
    ("Turn off this section's filters", "Tắt bộ lọc của phần này"),
    ("Remove this filter", "Bỏ bộ lọc này"),
    ("Reset all ({})", "Đặt lại tất cả ({})"),
    ("Turn off every filter", "Tắt toàn bộ bộ lọc"),
    ("Click to sort by {}.", "Bấm để sắp xếp theo {}."),
    ("☰ Columns", "☰ Cột"),
    ("Show or hide columns", "Hiện hoặc ẩn cột"),
    ("Dismiss", "Bỏ qua"),
    ("Undo", "Hoàn tác"),
    ("Back to {}", "Quay lại {}"),
    // Collection: the page and its three sub-tabs.
    ("📚 Collection", "📚 Bộ sưu tập"),
    ("🐴 Umas  {}", "🐴 Uma  {}"),
    ("The umas you own", "Các uma bạn sở hữu"),
    ("🃏 Support cards  {}", "🃏 Support card  {}"),
    ("The support cards you own", "Các support card bạn sở hữu"),
    ("⚡ Skills", "⚡ Skill"),
    (
        "Every skill in the game, and which ones you can get",
        "Mọi skill trong game, và những skill bạn có thể có",
    ),
    ("＋ Add cards", "＋ Thêm card"),
    (
        "Add cards you just pulled, or raise the limit break and level of cards you have",
        "Thêm card vừa quay được, hoặc nâng limit break và level của card bạn đang có",
    ),
    ("＋ Add umas", "＋ Thêm uma"),
    (
        "Click the umas you own, and set their stars and potential",
        "Bấm chọn các uma bạn sở hữu, rồi đặt số sao và potential",
    ),
    (
        "Click an uma to see her aptitudes, stats, skills and events. Hover one for the numbers; filter on the left, e.g. A or better at Long.",
        "Bấm một uma để xem aptitude, chỉ số, skill và sự kiện của cô ấy. Rê chuột để xem nhanh các con số; lọc ở bên trái, ví dụ A trở lên ở Long.",
    ),
    (
        "Click a card for its effects at your level, the skills it can teach and its events. Hover one to use it in your plan or compare two.",
        "Bấm một card để xem hiệu ứng ở level của bạn, các skill nó có thể dạy và sự kiện của nó. Rê chuột để dùng trong kế hoạch hoặc so sánh hai card.",
    ),
    (
        "Every skill on the Global server, and the umas and cards you own that give it. Pick a course on the left to see which can activate there.",
        "Mọi skill trên server Global, cùng các uma và card bạn sở hữu có thể cho skill đó. Chọn một cự ly ở bên trái để xem skill nào kích hoạt được ở đó.",
    ),
    // Collection: the shared kit.
    ("Show the filters", "Hiện bộ lọc"),
    ("{} filter{} on", "{0} bộ lọc{1} đang bật"),
    ("{}: click to show the filters", "{}: bấm để hiện bộ lọc"),
    ("Filters", "Bộ lọc"),
    (
        "Fold the filters away, for more room",
        "Thu gọn bộ lọc để có thêm chỗ",
    ),
    ("Clear the search", "Xóa tìm kiếm"),
    ("Quick views", "Chế độ xem nhanh"),
    (
        "One click sets the filters below. Click it again to clear them. Save your own with the link underneath.",
        "Một bấm để đặt bộ lọc bên dưới. Bấm lại để xóa. Lưu bộ lọc của riêng bạn bằng liên kết phía dưới.",
    ),
    (
        "Your saved view. Right-click to delete it.",
        "Chế độ xem bạn đã lưu. Bấm chuột phải để xóa.",
    ),
    ("🗑 Delete this view", "🗑 Xóa chế độ xem này"),
    ("Name this view", "Đặt tên chế độ xem"),
    ("Save", "Lưu"),
    ("Cancel", "Hủy"),
    (
        "＋ Save these filters as a view…",
        "＋ Lưu bộ lọc này thành chế độ xem…",
    ),
    (
        "Keep the filters you've set as a one-click view",
        "Lưu bộ lọc bạn đã đặt thành chế độ xem một bấm",
    ),
    ("Sort", "Sắp xếp"),
    (
        "Highest first (Z to A for names). Click for lowest first.",
        "Cao nhất trước (Z đến A với tên). Bấm để thấp nhất trước.",
    ),
    (
        "Lowest first (A to Z for names). Click for highest first.",
        "Thấp nhất trước (A đến Z với tên). Bấm để cao nhất trước.",
    ),
    ("All {0} {1}", "Tất cả {0} {1}"),
    ("Showing {0} of {1} {2}", "Hiện {0} / {1} {2}"),
    ("umas", "uma"),
    ("support cards", "support card"),
    (
        "Small pictures: more on screen",
        "Ảnh nhỏ: hiện được nhiều hơn",
    ),
    ("Medium pictures", "Ảnh vừa"),
    ("Large pictures", "Ảnh lớn"),
    // Collection: Umas.
    ("any", "bất kỳ"),
    ("A or better", "A trở lên"),
    ("Has an evolved skill", "Có skill tiến hóa"),
    ("Released in the last 60 days", "Ra mắt trong 60 ngày qua"),
    (
        "🔍 Search name, outfit or note (Ctrl+F)",
        "🔍 Tìm tên, outfit hoặc ghi chú (Ctrl+F)",
    ),
    (
        "Maxed: every potential skill unlocked",
        "Max: mọi skill potential đã mở khóa",
    ),
    ("P5 · maxed", "P5 · max"),
    ("{} uma{}", "{0} uma{1}"),
    ("Plan a career with her", "Lên kế hoạch career với cô ấy"),
    (
        "📋 Plan a career with her",
        "📋 Lên kế hoạch career với cô ấy",
    ),
    ("🔍 Open details", "🔍 Mở chi tiết"),
    (" (maxed)", " (max)"),
    ("A or better: {}", "A trở lên: {}"),
    ("Growth: {}", "Tăng trưởng: {}"),
    (
        "Click for details · right-click for more",
        "Bấm để xem chi tiết · chuột phải để biết thêm",
    ),
    (
        "{} aptitude: S is best, then A, B, C… down to G",
        "Aptitude {}: S tốt nhất, rồi A, B, C… đến G",
    ),
    (
        "Growth: a bonus to what training adds to {}",
        "Growth: phần cộng thêm vào chỉ số {} khi train",
    ),
    ("⧉ Copy as CSV", "⧉ Sao chép CSV"),
    ("▦ Pictures", "▦ Hình ảnh"),
    ("Show art tiles", "Hiện dạng ảnh"),
    ("☰ Table", "☰ Bảng"),
    (
        "Show a sortable table with more columns",
        "Hiện bảng sắp xếp được với nhiều cột hơn",
    ),
    (
        "Copy the rows shown, with the columns shown, to paste into a spreadsheet",
        "Sao chép các dòng và cột đang hiện để dán vào bảng tính",
    ),
    (
        "Copied {} rows. Paste them into a spreadsheet.",
        "Đã sao chép {0} dòng. Dán vào bảng tính.",
    ),
    ("Close", "Đóng"),
    ("Potential 5 (maxed)", "Potential 5 (max)"),
    (
        "All her potential skills are unlocked",
        "Mọi skill potential của cô ấy đã được mở khóa",
    ),
    ("Potential {} of 5", "Potential {} trên 5"),
    (
        "Some of her potential skills are still locked",
        "Một số skill potential của cô ấy vẫn bị khóa",
    ),
    ("On Global since launch", "Có trên Global từ ngày ra mắt"),
    (
        "She was there when the Global server opened, {}.",
        "Cô ấy có mặt từ khi server Global mở, {}.",
    ),
    ("Released {}", "Ra mắt {}"),
    (
        "When she arrived on the Global server.",
        "Khi cô ấy xuất hiện trên server Global.",
    ),
    (
        "Open the Planner with this uma as the trainee",
        "Mở Planner với uma này làm trainee",
    ),
    (
        "No knowledge data for this outfit yet. Rebuild knowledge/data.",
        "Chưa có dữ liệu knowledge cho outfit này. Hãy tạo lại knowledge/data.",
    ),
    ("Base", "Cơ bản"),
    ("INNATE", "INNATE"),
    ("POTENTIAL {}{}", "POTENTIAL {0}{1}"),
    ("  🔒 locked", "  🔒 đã khóa"),
    ("FROM HER EVENTS", "TỪ SỰ KIỆN CỦA CÔ ẤY"),
    ("Needs: {}", "Cần: {}"),
    ("Events ({})", "Sự kiện ({})"),
    ("Option {}", "Lựa chọn {}"),
    ("Outcome", "Kết quả"),
    (
        "Your {} cards can't go in her own deck.",
        "Card {} của bạn không thể nằm trong deck của chính cô ấy.",
    ),
    ("S only", "Chỉ S"),
    ("any grade", "mọi hạng"),
    (
        "{0}: {1}. Click to cycle any → A or better → S only; right-click to clear.",
        "{0}: {1}. Bấm để chuyển mọi hạng → A trở lên → chỉ S; chuột phải để xóa.",
    ),
    ("Surface", "Mặt sân"),
    ("Distance", "Cự ly"),
    ("Style", "Style"),
    ("Track", "Đường đua"),
    ("Aptitudes", "Aptitude"),
    (
        "S and A are ideal. Below A she runs slower there, unless parents' pink sparks raise the grade.",
        "S và A là lý tưởng. Dưới A cô ấy chạy chậm hơn ở đó, trừ khi spark hồng từ parent nâng hạng.",
    ),
    ("Skill", "Skill"),
    ("Growth", "Tăng trưởng"),
    ("Stats", "Chỉ số"),
    (
        "Base: her starting stats. Growth: a bonus to what training adds to that stat.",
        "Base: chỉ số khởi đầu của cô ấy. Growth: phần cộng thêm vào chỉ số đó khi train.",
    ),
    ("Skills", "Skill"),
    (
        "Click a skill to see what it does. Potential skills above her current potential are locked.",
        "Bấm một skill để xem tác dụng. Skill potential trên mức potential hiện tại của cô ấy sẽ bị khóa.",
    ),
    ("Evolved skills", "Skill tiến hóa"),
    (
        "Stronger versions of her skills, unlocked in a career when the conditions are met.",
        "Phiên bản mạnh hơn của skill cô ấy, mở khóa trong career khi đủ điều kiện.",
    ),
    ("Career objectives", "Mục tiêu career"),
    (
        "Goals she must meet during her career. Missing one ends the career early.",
        "Các mục tiêu cô ấy phải đạt trong career. Trượt một mục tiêu sẽ kết thúc career sớm.",
    ),
    (
        "Story events in her career and what each choice gives. Click one to open it.",
        "Sự kiện cốt truyện trong career của cô ấy và phần thưởng của từng lựa chọn. Bấm để mở.",
    ),
    ("Related", "Liên quan"),
    ("Maxed potential", "Potential tối đa"),
    (
        "Umas at P5: every skill of their own unlocked",
        "Uma đạt P5: mọi skill của cô ấy đã mở khóa",
    ),
    ("Needs potential", "Cần nâng potential"),
    (
        "Umas below P5: potential still to raise",
        "Uma dưới P5: vẫn còn potential để nâng",
    ),
    ("Long A+", "Long A+"),
    (
        "Umas with A or better at Long distance",
        "Uma đạt A trở lên ở cự ly Long",
    ),
    ("Dirt A+", "Dirt A+"),
    (
        "Umas with A or better on dirt",
        "Uma đạt A trở lên trên sân Dirt",
    ),
    ("Stars", "Sao"),
    (
        "Star rank, 1–5★. Higher stars make an uma stronger, including her unique skill.",
        "Hạng sao, 1–5★. Sao cao hơn giúp uma mạnh hơn, kể cả skill Unique của cô ấy.",
    ),
    ("Potential", "Potential"),
    (
        "Potential unlocks an uma's own skills, P1 to P5. At P5 all of them can be bought.",
        "Potential mở khóa các skill riêng của uma, P1 đến P5. Ở P5 có thể mua tất cả.",
    ),
    (
        "Click a cell to keep umas graded A or better there; click again for S only; once more to clear. Grades go from S (best) through A, B, C… down to G.",
        "Bấm một ô để giữ các uma đạt A trở lên ở đó; bấm lần nữa để chỉ giữ S; bấm thêm để xóa. Hạng từ S (tốt nhất) qua A, B, C… đến G.",
    ),
    ("More", "Thêm"),
    (
        "Umas whose skills can evolve into stronger versions during a career",
        "Uma có skill có thể tiến hóa thành phiên bản mạnh hơn trong career",
    ),
    (
        "Outfits that came out on Global in the last two months",
        "Outfit ra mắt trên Global trong hai tháng qua",
    ),
    ("No umas match", "Không có uma nào khớp"),
    ("Loosen or clear the filters.", "Nới lỏng hoặc xóa bộ lọc."),
    // Collection: cards.
    ("Type, then rarity and LB", "Loại, rồi độ hiếm và LB"),
    ("Rarity", "Độ hiếm"),
    ("Limit break", "Limit break"),
    ("Level", "Level"),
    ("Card title", "Tên card"),
    ("Release date", "Ngày phát hành"),
    ("Number of skill hints", "Số skill hint"),
    ("Name", "Tên"),
    ("Outfit", "Outfit"),
    (
        "Best fit for the aptitude filter",
        "Phù hợp nhất với bộ lọc aptitude",
    ),
    ("Turf aptitude", "Aptitude Turf"),
    ("Dirt aptitude", "Aptitude Dirt"),
    ("Sprint aptitude", "Aptitude Sprint"),
    ("Mile aptitude", "Aptitude Mile"),
    ("Medium aptitude", "Aptitude Medium"),
    ("Long aptitude", "Aptitude Long"),
    ("Front Runner aptitude", "Aptitude Front Runner"),
    ("Pace Chaser aptitude", "Aptitude Pace Chaser"),
    ("Late Surger aptitude", "Aptitude Late Surger"),
    ("End Closer aptitude", "Aptitude End Closer"),
    ("Speed growth", "Growth Speed"),
    ("Stamina growth", "Growth Stamina"),
    ("Power growth", "Growth Power"),
    ("Guts growth", "Growth Guts"),
    ("Wit growth", "Growth Wit"),
    ("Unique skill", "Skill Unique"),
    ("Uma", "Uma"),
    ("Released", "Ra mắt"),
    (
        "P1 to P5: how many of her own skills are unlocked",
        "P1 đến P5: số skill riêng của cô ấy đã mở khóa",
    ),
    ("Her own skill, from 1★", "Skill riêng của cô ấy, từ 1★"),
    (
        "When this outfit came out on Global",
        "Khi outfit này ra mắt trên Global",
    ),
    ("No cards match", "Không có card nào khớp"),
    (
        "🔍 Search card, uma or note (Ctrl+F)",
        "🔍 Tìm card, uma hoặc ghi chú (Ctrl+F)",
    ),
    ("Maxed SSRs", "SSR max"),
    (
        "SSR cards at max limit break (MLB)",
        "Card SSR đạt limit break tối đa (MLB)",
    ),
    ("Not maxed", "Chưa max"),
    (
        "Cards below MLB: more copies would make them stronger",
        "Card dưới MLB: thêm bản sao sẽ giúp mạnh hơn",
    ),
    ("Teaches a gold", "Dạy skill Gold"),
    (
        "Cards that can teach a gold skill, through a hint or an event",
        "Card có thể dạy skill Gold qua hint hoặc sự kiện",
    ),
    ("Type and rarity", "Loại và độ hiếm"),
    (
        "Which stat the card trains (Pal and Group cards work differently), and its rarity: SSR is the strongest, then SR, then R.",
        "Card train chỉ số nào (card Pal và Group hoạt động khác), và độ hiếm: SSR mạnh nhất, rồi SR, rồi R.",
    ),
    (
        "Merging copies of a card raises its level cap: LB0 to LB4. LB4, called MLB (max limit break), is the card at full strength. The diamonds under each card show its LB.",
        "Gộp bản sao của card để nâng giới hạn level: LB0 đến LB4. LB4, gọi là MLB (max limit break), là card ở trạng thái mạnh nhất. Các viên kim cương dưới mỗi card hiện LB của nó.",
    ),
    ("Below level cap", "Dưới giới hạn level"),
    (
        "Cards you could still level up at their current limit break",
        "Card vẫn có thể nâng level ở limit break hiện tại",
    ),
    ("Teaches…", "Dạy…"),
    (
        "Cards that can teach any of these skills, through a hint or an event. Type part of a skill's name and pick it.",
        "Card có thể dạy bất kỳ skill nào trong số này qua hint hoặc sự kiện. Nhập một phần tên skill rồi chọn.",
    ),
    ("💡 Type a skill name", "💡 Nhập tên skill"),
    (
        "None of your cards teach a skill like that.",
        "Không card nào của bạn dạy skill như vậy.",
    ),
    ("Stop filtering by this skill", "Bỏ lọc theo skill này"),
    (
        "Cards that teach any of these.",
        "Card dạy bất kỳ skill nào trong số này.",
    ),
    ("Gold skills only", "Chỉ skill Gold"),
    (
        "Only cards that can teach a gold skill; the box above then suggests gold skills only",
        "Chỉ card có thể dạy skill Gold; ô phía trên khi đó chỉ gợi ý skill Gold",
    ),
    ("Effect minimums", "Mức tối thiểu của hiệu ứng"),
    (
        "Cards whose effect, at the level you own them, is at least this much. For example Friendship ≥ 25.",
        "Card có hiệu ứng, ở level bạn sở hữu, đạt ít nhất mức này. Ví dụ Friendship ≥ 25.",
    ),
    ("Remove this minimum", "Xóa mức tối thiểu này"),
    ("＋ Add a minimum", "＋ Thêm mức tối thiểu"),
    ("Scenario link", "Liên kết scenario"),
    (
        "In some training scenarios, cards of certain characters (linked characters) get a bonus, and some teach a gold skill through the scenario's events. From knowledge/scenarios.json.",
        "Trong một số scenario train, card của nhân vật nhất định (nhân vật liên kết) được bonus, và một số dạy skill Gold qua sự kiện của scenario. Từ knowledge/scenarios.json.",
    ),
    ("Any scenario", "Mọi scenario"),
    ("Linked in {}", "Liên kết trong {}"),
    ("Linked in {} ({} umas)", "Liên kết trong {0} ({1} uma)"),
    (
        "Max limit break (LB4): the card at full strength",
        "Max limit break (LB4): card ở trạng thái mạnh nhất",
    ),
    ("Limit break {} of 4", "Limit break {0} trên 4"),
    ("MLB · max limit break", "MLB · max limit break"),
    ("{} card{}", "{0} card{1}"),
    (
        "Use in the current plan's deck",
        "Dùng trong deck của kế hoạch hiện tại",
    ),
    ("Stop comparing this card", "Ngừng so sánh card này"),
    (
        "Compare: pick two cards to see their effects side by side",
        "So sánh: chọn hai card để xem hiệu ứng cạnh nhau",
    ),
    (
        "📋 Use in the current plan",
        "📋 Dùng trong kế hoạch hiện tại",
    ),
    ("⇄ Stop comparing", "⇄ Ngừng so sánh"),
    ("⇄ Compare with another card", "⇄ So sánh với card khác"),
    ("{} {} · {} · level {} of {}", "{} {} · {} · level {} / {}"),
    ("Teaches {} skills{}", "Dạy {} skill{}"),
    ("{} skills{}", "{} skill{}"),
    ("Teaches {}", "Dạy {}"),
    ("{} ≥ {}", "{} ≥ {}"),
    ("{} {} · released {}", "{} {} · ra mắt {}"),
    ("Level {} / {}", "Level {} / {}"),
    ("Max limit break (MLB)", "Max limit break (MLB)"),
    ("UNIQUE", "UNIQUE"),
    (" ({} gold)", " ({} Gold)"),
    ("Unique effect: {}", "Hiệu ứng Unique: {}"),
    ("The card's title", "Tên card"),
    (
        "SSR is the strongest, then SR, then R",
        "SSR mạnh nhất, rồi SR, rồi R",
    ),
    ("Which stat the card trains", "Card train chỉ số nào"),
    (
        "Limit break: LB0 to LB4 (MLB, full strength)",
        "Limit break: LB0 đến LB4 (MLB, mạnh nhất)",
    ),
    (
        "Level, of the cap at this limit break",
        "Level, so với giới hạn ở limit break này",
    ),
    (
        "{}{}, at the level you own the card",
        "{0}{1}, ở level bạn sở hữu card",
    ),
    (
        "How many skills it can offer as hints",
        "Số skill có thể cung cấp dưới dạng hint",
    ),
    (
        "The extra effect only this card has, and the level it starts at",
        "Hiệu ứng thêm chỉ card này có, và level bắt đầu có",
    ),
    ("Card", "Card"),
    ("Type", "Loại"),
    ("LB", "LB"),
    ("Hints", "Hint"),
    ("Unique effect", "Hiệu ứng Unique"),
    (
        "{} levels below the cap at this limit break",
        "{} level dưới giới hạn ở limit break này",
    ),
    (
        "{}\nNot active yet at your level",
        "{0}\nChưa kích hoạt ở level của bạn",
    ),
    (
        "{} is already in your plan's deck.",
        "{} đã có trong deck kế hoạch của bạn.",
    ),
    (
        "Can't: {} is the trainee, and her own card can't go in her deck.",
        "Không được: {} là trainee, card của chính cô ấy không thể nằm trong deck của cô ấy.",
    ),
    (
        "Your plan's deck already has a {} card; a deck can't hold two cards of the same uma.",
        "Deck kế hoạch của bạn đã có card {}; một deck không thể chứa hai card của cùng một uma.",
    ),
    (
        "Added {} to slot {} of your plan's deck. Career → Plan shows it.",
        "Đã thêm {} vào ô {} trong deck kế hoạch. Career → Kế hoạch sẽ hiện nó.",
    ),
    (
        "Your plan's deck is full. Open Career → Plan to swap a card.",
        "Deck kế hoạch đã đầy. Mở Career → Kế hoạch để đổi card.",
    ),
    ("⇄ Compare", "⇄ So sánh"),
    (
        "{} with… pick another card's ⇄ (hover a tile), or right-click it.",
        "{} với… chọn ⇄ của card khác (rê chuột lên card), hoặc bấm chuột phải.",
    ),
    ("✕ Cancel", "✕ Hủy"),
    ("Compare cards", "So sánh card"),
    ("✖ Close", "✖ Đóng"),
    (
        "Effects at the level you own each card. The higher value is bold.",
        "Hiệu ứng ở level bạn sở hữu từng card. Giá trị cao hơn được in đậm.",
    ),
    ("Skill hints", "Skill hint"),
    ("\nGold: {}", "\nGold: {}"),
    ("Add cards", "Thêm card"),
    (
        "Click a card to set its limit break and level, or click a diamond on a card to save it at that LB.",
        "Bấm một card để đặt limit break và level, hoặc bấm viên kim cương trên card để lưu ở LB đó.",
    ),
    (
        "🔍 Search uma, title or skill hint",
        "🔍 Tìm uma, tên card hoặc skill hint",
    ),
    ("All types", "Mọi loại"),
    ("Owned only", "Chỉ card đã có"),
    (
        "Only cards you have, to raise their LB or level",
        "Chỉ card bạn đã có, để nâng LB hoặc level",
    ),
    ("Not on Global yet", "Chưa có trên Global"),
    (
        "uma.guide lists some cards ahead of their Global release",
        "uma.guide liệt kê một số card trước khi ra mắt trên Global",
    ),
    ("{} cards, newest first", "{} card, mới nhất trước"),
    ("Pick a card on the left.", "Chọn một card ở bên trái."),
    ("Not owned", "Chưa sở hữu"),
    ("Owned: LB{} · Lv {}", "Đã có: LB{} · Lv {}"),
    (
        "{} {} [{}]\n{} card · released {}\n{}\nClick to edit · click a diamond to save at that LB",
        "{} {} [{}]\nCard {} · ra mắt {}\n{}\nBấm để sửa · bấm viên kim cương để lưu ở LB đó",
    ),
    ("Notes (optional)", "Ghi chú (tùy chọn)"),
    ("💾 Update card", "💾 Cập nhật card"),
    ("＋ Add to roster", "＋ Thêm vào roster"),
    ("UmaYu folder not found.", "Không tìm thấy thư mục UmaYu."),
    ("Pick a card first.", "Hãy chọn một card trước."),
    ("No data for that card.", "Không có dữ liệu cho card đó."),
    ("Unknown card type {}", "Loại card không xác định {}"),
    ("Couldn't read {}: {}", "Không đọc được {}: {}"),
    ("Couldn't write {}: {}", "Không ghi được {}: {}"),
    ("{} {} [{}] at {} Lv{}.", "{} {} [{}] ở {} Lv{}."),
    ("Updated", "Đã cập nhật"),
    ("Added", "Đã thêm"),
    (
        " Downloading its art from uma.guide…",
        " Đang tải ảnh từ uma.guide…",
    ),
    (
        "Saved, but the card art couldn't be downloaded; it shows as a placeholder.",
        "Đã lưu, nhưng không tải được ảnh card; ảnh sẽ hiện dạng placeholder.",
    ),
    ("Owned · LB{} Lv{}", "Đã có · LB{} Lv{}"),
    ("The cap at LB{} is {}.", "Giới hạn ở LB{} là {}."),
    ("Skill hints ({})", "Skill hint ({})"),
    ("Skills from events", "Skill từ sự kiện"),
    ("No outcome data.", "Chưa có dữ liệu kết quả."),
    (
        "One of these, at random (odds not in the data):",
        "Một trong số này, ngẫu nhiên (dữ liệu không có tỷ lệ):",
    ),
    ("Other {} cards", "Card {} khác"),
    (
        "No knowledge data for this card yet. Rebuild knowledge/data.",
        "Chưa có dữ liệu knowledge cho card này. Hãy tạo lại knowledge/data.",
    ),
    ("Support effects", "Hiệu ứng support"),
    (
        "What the card adds to training. Green numbers in the last column are what you'd gain by maxing it.",
        "Card cộng thêm gì khi train. Số màu xanh ở cột cuối là phần bạn được thêm nếu max card.",
    ),
    ("Effect", "Hiệu ứng"),
    ("Yours · Lv {}", "Của bạn · Lv {}"),
    ("MLB · Lv {}", "MLB · Lv {}"),
    (
        "An extra effect only this card has, once it reaches the level shown.",
        "Hiệu ứng thêm chỉ card này có, khi đạt level hiển thị.",
    ),
    ("Active · from Lv {}", "Kích hoạt · từ Lv {}"),
    ("Locked until Lv {}", "Khóa đến Lv {}"),
    (
        "{} levels below this LB's cap",
        "{} level dưới giới hạn của LB này",
    ),
    (
        "Not usable when training {}.",
        "Không dùng được khi train {}.",
    ),
    ("📋 Use in plan", "📋 Dùng trong kế hoạch"),
    (
        "Put it in the current plan's deck",
        "Đưa vào deck của kế hoạch hiện tại",
    ),
    ("⇄ Comparing", "⇄ Đang so sánh"),
    ("⇄ Compare", "⇄ So sánh"),
    (
        "Pick it, then another card, to see their effects side by side",
        "Chọn card này, rồi card khác, để xem hiệu ứng cạnh nhau",
    ),
    // Collection: skills.
    ("No skill data", "Không có dữ liệu skill"),
    ("No skills match", "Không có skill nào khớp"),
    (
        "Loosen the filters, or switch to All skills.",
        "Nới lỏng bộ lọc, hoặc chuyển sang Mọi skill.",
    ),
    (
        "🔍 Search name or effect (Ctrl+F)",
        "🔍 Tìm tên hoặc hiệu ứng (Ctrl+F)",
    ),
    ("Golds", "Skill Gold"),
    (
        "Gold skills: stronger, and usually need their white version first",
        "Skill Gold: mạnh hơn, và thường cần mua bản White trước",
    ),
    ("Recovery", "Hồi máu"),
    ("Skills that restore HP", "Skill hồi HP"),
    ("Speed boosts", "Tăng Speed"),
    ("Skills that raise speed", "Skill tăng Speed"),
    (
        "White skills are the ordinary ones. Gold skills are stronger and usually need their white version first. Unique and evolved skills belong to particular umas.",
        "Skill White là skill thường. Skill Gold mạnh hơn và thường cần bản White trước. Skill Unique và tiến hóa thuộc về từng uma cụ thể.",
    ),
    ("What it does", "Tác dụng"),
    (
        "Speed Boost raises speed; Acceleration helps reach top speed sooner; Recovery restores HP; Passive raises stats; Debuff hinders rivals; Lane Effect and Vision change how she moves through the field.",
        "Speed Boost tăng Speed; Acceleration giúp đạt tốc độ tối đa sớm hơn; Recovery hồi HP; Passive tăng chỉ số; Debuff cản trở đối thủ; Lane Effect và Vision thay đổi cách cô ấy di chuyển trên đường đua.",
    ),
    ("Works for", "Phù hợp với"),
    (
        "Many skills only work for one running style, distance or surface. Pick one to hide skills locked to something else. Click again to clear.",
        "Nhiều skill chỉ dùng được cho một running style, cự ly hoặc mặt sân. Chọn một để ẩn skill bị khóa cho thứ khác. Bấm lại để xóa.",
    ),
    ("Course", "Cự ly đua"),
    (
        "Pick a Champions Meeting or a course to keep only skills whose conditions can be met there, by the race simulator's own check. A CM also marks the skills it turns off.",
        "Chọn một Champions Meeting hoặc một cự ly để chỉ giữ skill có điều kiện thỏa được ở đó, theo chính bước kiểm tra của trình mô phỏng. CM cũng đánh dấu các skill bị tắt.",
    ),
    (
        "The simulator's data isn't loaded.",
        "Chưa tải dữ liệu của trình mô phỏng.",
    ),
    ("📋 Your plan's course", "📋 Cự ly của kế hoạch"),
    ("your plan's course", "cự ly của kế hoạch"),
    ("Any course…", "Mọi cự ly…"),
    ("Conditions", "Điều kiện"),
    (
        "Checking which skills can activate on {}…",
        "Đang kiểm tra skill nào kích hoạt được trên {}…",
    ),
    (
        " Struck-through skills are turned off in the {}.",
        " Skill bị gạch ngang bị tắt trong {}.",
    ),
    (
        "Only skills that can activate on {} (simulator check).{}",
        "Chỉ skill có thể kích hoạt trên {} (trình mô phỏng kiểm tra).{}",
    ),
    ("Ones I can get ({available})", "Skill tôi có thể lấy ({0})"),
    ("All skills ({total})", "Mọi skill ({0})"),
    ("{} skills", "{} skill"),
    ("Showing {}", "Hiện {}"),
    ("For {}", "Dành cho {}"),
    ("Activates on {}", "Kích hoạt trên {}"),
    ("Game order", "Thứ tự trong game"),
    ("SP cost", "Giá SP"),
    ("Sources you own", "Nguồn bạn sở hữu"),
    (
        "Gain on the plan's course",
        "Lợi ích trên cự ly của kế hoạch",
    ),
    (
        "Its colour shows its rarity: white, gold, or pink for unique and evolved",
        "Màu cho biết độ hiếm: trắng, vàng, hoặc hồng cho Unique và tiến hóa",
    ),
    (
        "Skill points it costs before hint discounts",
        "Điểm skill phải trả trước khi giảm giá nhờ hint",
    ),
    ("Its effect at base strength", "Hiệu ứng ở mức cơ bản"),
    ("When", "Khi nào"),
    (
        "When it triggers, from uma.guide's summary. Hover a cell for all of it.",
        "Khi nào kích hoạt, theo tóm tắt của uma.guide. Rê chuột lên ô để xem đầy đủ.",
    ),
    ("Only for", "Chỉ dành cho"),
    (
        "The running style, distance or surface it's locked to; blank means any",
        "Running style, cự ly hoặc mặt sân bị khóa; trống nghĩa là mọi",
    ),
    ("From", "Từ"),
    (
        "Umas and cards you own that can give you this skill. Hover for names.",
        "Uma và card bạn sở hữu có thể cho skill này. Rê chuột để xem tên.",
    ),
    ("Gain", "Lợi ích"),
    (
        "Lengths it gains on your plan's course, from the planner's skill simulation",
        "Số length được thêm trên cự ly của kế hoạch, theo mô phỏng skill của planner",
    ),
    ("What the skill does", "Skill làm gì"),
    (
        "White, gold, unique, inherited unique (from a parent) or evolved",
        "White, Gold, Unique, inherited unique (từ parent) hoặc tiến hóa",
    ),
    (
        "Turned off in the {}: it won't activate there.",
        "Bị tắt trong {}: sẽ không kích hoạt ở đó.",
    ),
    (
        "The simulator can't check this skill (most are scenario skills), so it stays in the list.",
        "Trình mô phỏng không kiểm tra được skill này (phần lớn là skill scenario), nên nó vẫn ở trong danh sách.",
    ),
    ("Wit check", "Kiểm tra Wit"),
    (
        "Whether it triggers depends partly on Wit",
        "Việc kích hoạt một phần phụ thuộc vào Wit",
    ),
    ("{} rating", "{} rating"),
    ("On {} ({} m)", "Trên {} ({} m)"),
    (
        "Evaluation points: what it adds to an uma's rank when learned (uma.guide's data). The game also scales skills tied to a distance or style by her aptitude.",
        "Điểm đánh giá: phần cộng vào rank của uma khi học (dữ liệu uma.guide). Game còn nhân skill gắn với cự ly hoặc style theo aptitude của cô ấy.",
    ),
    ("Tier list: CM {} · TT {}", "Tier list: CM {} · TT {}"),
    ("Only for: {}", "Chỉ dành cho: {}"),
    ("⚠ Turned off in the {}.", "⚠ Bị tắt trong {}."),
    ("the Race sim's course", "cự ly của Race sim"),
    ("🏁 Test on a course", "🏁 Thử trên một cự ly"),
    (
        "Open the Race sim on {}: Uma 1 without this skill, Uma 2 with it",
        "Mở Race sim trên {}: Uma 1 không có skill này, Uma 2 có",
    ),
    (
        "Lasts {} s per 1000 m: a race scales it by its distance ÷ 1000, as the simulator does.",
        "Kéo dài {} s mỗi 1000 m: cuộc đua nhân theo cự ly ÷ 1000, như trình mô phỏng.",
    ),
    ("Once per race", "Một lần mỗi cuộc đua"),
    (
        "Cooldown {} s (base, before the distance scaling)",
        "Hồi chiêu {} s (cơ bản, trước khi nhân theo cự ly)",
    ),
    (
        "WHEN IT TRIGGERS (uma.guide)",
        "KHI NÀO KÍCH HOẠT (uma.guide)",
    ),
    ("IN-GAME DESCRIPTION", "MÔ TẢ TRONG GAME"),
    ("Raw conditions", "Điều kiện gốc"),
    ("conditions: {}", "điều kiện: {}"),
    ("precondition: {}", "điều kiện trước: {}"),
    ("second activation: {}", "kích hoạt lần hai: {}"),
    ("Upgrade path", "Đường nâng cấp"),
    (
        "A gold (or ◎) skill builds on its white (or ○) version: you buy the lower one first, and its cost counts toward the upgrade.",
        "Skill Gold (hoặc ◎) nâng từ bản White (hoặc ○): mua bản thấp trước, chi phí của nó được tính vào lần nâng cấp.",
    ),
    ("Where you can get it", "Lấy ở đâu"),
    (
        "Nothing you own right now.",
        "Hiện tại bạn chưa có nguồn nào.",
    ),
    ("🔒 LOCKED BEHIND POTENTIAL", "🔒 KHÓA SAU POTENTIAL"),
    ("Sources you don't own ({})", "Nguồn bạn chưa sở hữu ({})"),
    ("the {}", "{}"),
    ("Unique of", "Unique của"),
    ("Innate for", "Bẩm sinh của"),
    ("Potential skill of", "Skill potential của"),
    ("Event of", "Sự kiện của"),
    ("Evolution for", "Tiến hóa cho"),
    ("Hint from", "Hint từ"),
    (" (P{})", " (P{})"),
    (" (from {})", " (từ {})"),
    (
        "Race sim: Uma 2 is Uma 1 plus {}. Press ▶ Run to see what it's worth.",
        "Race sim: Uma 2 là Uma 1 cộng {}. Bấm ▶ Chạy để xem đáng giá bao nhiêu.",
    ),
    // Phase 5: Career and the planner.
    (
        "Plan it → race it → log what happened",
        "Lên kế hoạch → cho đua → ghi lại kết quả",
    ),
    // Career → Plan: the saved plans.
    (
        "New plan: start an empty one. Save the current one first if you want to keep it.",
        "Kế hoạch mới: bắt đầu một kế hoạch trống. Hãy lưu kế hoạch hiện tại trước nếu muốn giữ lại.",
    ),
    (
        "Start an empty plan. Save the current one first if you want to keep it.",
        "Bắt đầu một kế hoạch trống. Hãy lưu kế hoạch hiện tại trước nếu muốn giữ lại.",
    ),
    ("Saved plans", "Kế hoạch đã lưu"),
    (
        "No saved plans yet. Plans you save land in the plans folder.",
        "Chưa có kế hoạch nào được lưu. Kế hoạch bạn lưu sẽ nằm trong thư mục plans.",
    ),
    (
        "Saved plans ({}): click to pick one",
        "Kế hoạch đã lưu ({}): bấm để chọn một kế hoạch",
    ),
    ("＋ New plan", "＋ Kế hoạch mới"),
    (
        "Plans you saved, newest first. Click one to open it. They live in the plans folder as markdown files.",
        "Kế hoạch bạn đã lưu, mới nhất trước. Bấm để mở. Chúng nằm trong thư mục plans dưới dạng file markdown.",
    ),
    (
        "That plan has no app data block (written by hand?). Read it in ⚙ Settings → Data and updates → Browse notes and data files.",
        "Kế hoạch đó không có block dữ liệu của app (viết tay?). Đọc nó ở ⚙ Cài đặt → Dữ liệu và cập nhật → Duyệt ghi chú và file dữ liệu.",
    ),
    // Career → Plan: the title and the steps.
    (
        "Untitled plan (click to name it)",
        "Kế hoạch chưa đặt tên (bấm để đặt tên)",
    ),
    ("💾 Save plan", "💾 Lưu kế hoạch"),
    (
        "Saves it as plans/<date>-<uma>-<purpose>.md, so you can reopen it from the list on the left. Claude can then simulate it and write a recommendation into the file.",
        "Lưu thành plans/<date>-<uma>-<purpose>.md để bạn mở lại từ danh sách bên trái. Sau đó Claude có thể mô phỏng và ghi gợi ý vào file.",
    ),
    ("▶ Simulate", "▶ Mô phỏng"),
    (
        "Race the plan in the Race sim tab: the trainee with her skills to buy against the same build without them, on the plan's course and conditions.",
        "Cho kế hoạch chạy đua ở tab Mô phỏng đua: trainee cùng các skill cần mua đấu với chính build đó không có skill, trên đường đua và điều kiện của kế hoạch.",
    ),
    ("↗ Open Umalator", "↗ Mở Umalator"),
    (
        "Opens the umalator.app website with this build as Uma 1 and Uma 2, on the plan's course and conditions",
        "Mở trang umalator.app với build này ở Uma 1 và Uma 2, trên đường đua và điều kiện của kế hoạch",
    ),
    (
        "Opened Umalator with this build in both slots; change Uma 2 to compare. The link is also on the clipboard.",
        "Đã mở Umalator với build này ở cả hai ô; đổi Uma 2 để so sánh. Link cũng đã được sao chép.",
    ),
    (" Note: {}.", " Lưu ý: {}."),
    ("📋 Copy for Umalator", "📋 Copy cho Umalator"),
    (
        "Copies this build to paste into umalator.app: ▶ UMA → Uma 1 or 2 → import (↑) → Paste from clipboard",
        "Sao chép build này để dán vào umalator.app: ▶ UMA → Uma 1 hoặc 2 → import (↑) → Paste from clipboard",
    ),
    (
        "Build copied. In Umalator open ▶ UMA, pick Uma 1 or 2, then import (↑) → Paste from clipboard.",
        "Đã copy build. Trong Umalator mở ▶ UMA, chọn Uma 1 hoặc 2, rồi import (↑) → Paste from clipboard.",
    ),
    (
        " Left out (Umalator doesn't know them): {}.",
        " Bỏ qua (Umalator không biết các mục này): {}.",
    ),
    (
        "Work down the page in five steps: the goal, a trainee, her target stats, a deck of cards, and the skills to buy. Then save the plan.",
        "Làm lần lượt năm bước trên trang: mục tiêu, trainee, chỉ số mục tiêu, deck card và skill cần mua. Sau đó lưu kế hoạch.",
    ),
    ("Step 1 · Goal", "Bước 1 · Mục tiêu"),
    ("Parent run", "Lượt làm parent"),
    ("Other", "Khác"),
    ("or type, e.g. night race", "hoặc gõ, vd. night race"),
    (
        "The chips edit this text; anything else you type is kept.",
        "Các chip chỉnh sửa văn bản này; nội dung khác bạn gõ vẫn được giữ.",
    ),
    ("Sparks wanted", "Spark mong muốn"),
    ("e.g. Speed 3★, Turf, unique", "vd. Speed 3★, Turf, unique"),
    ("Step 2 · Trainee", "Bước 2 · Trainee"),
    (
        "Step 3 · Target stats and parents",
        "Bước 3 · Chỉ số mục tiêu và parent",
    ),
    ("📝 Notes", "📝 Ghi chú"),
    // Career → Plan: the goal and the meta sheet.
    ("Choose a trainee…", "Chọn trainee…"),
    (
        "SORTED BY APTITUDE FIT FOR THE GOAL ABOVE",
        "SẮP XẾP THEO APTITUDE PHÙ HỢP VỚI MỤC TIÊU Ở TRÊN",
    ),
    (
        "✨ Best fits for this goal:",
        "✨ Phù hợp nhất với mục tiêu này:",
    ),
    (
        "\n★ The meta sheet recommends her for this CM and style.",
        "\n★ Sheet meta gợi ý cô ấy cho CM và style này.",
    ),
    (
        "Below A: {}. Pink sparks from parents raise aptitudes.",
        "Dưới A: {}. Spark hồng từ parent giúp tăng aptitude.",
    ),
    ("doesn't fit this target", "không phù hợp mục tiêu này"),
    ("Pick a trainee first.", "Hãy chọn trainee trước."),
    (
        "The trainee isn't in your roster.",
        "Trainee không có trong roster.",
    ),
    (
        "No knowledge data for this trainee.",
        "Không có dữ liệu knowledge cho trainee này.",
    ),
    (
        "Enter target stats first.",
        "Hãy nhập chỉ số mục tiêu trước.",
    ),
    ("Couldn't save: {}", "Không lưu được: {}"),
    (
        "Anything Claude should know when simulating this plan",
        "Điều gì Claude cần biết khi mô phỏng kế hoạch này",
    ),
    (
        "Parent {} and its key sparks",
        "Parent {} và các spark chính",
    ),
    ("📂 Saved", "📂 Đã lưu"),
    (
        "None saved yet. Add parents to account/parents.md.",
        "Chưa lưu parent nào. Thêm parent vào account/parents.md.",
    ),
    (
        "✨ Pink sparks that would help: {}",
        "✨ Spark hồng sẽ hữu ích: {}",
    ),
    ("Upcoming CM", "CM sắp tới"),
    ("✔ announced\n? estimated", "✔ đã công bố\n? ước tính"),
    ("? estimated", "? ước tính"),
    (
        "✔: announced in the game's news. ?: not announced yet, so Umalator's guess at the date, course and conditions.",
        "✔: đã công bố trong tin tức game. ?: chưa công bố, nên Umalator tự ước tính ngày, đường đua và điều kiện.",
    ),
    (
        "none in knowledge/data/umalator.json; refresh it with tools/update_knowledge.py",
        "không có trong knowledge/data/umalator.json; làm mới bằng tools/update_knowledge.py",
    ),
    (
        "Click again to unlink it (the course and conditions stay).",
        "Bấm lại để bỏ liên kết (đường đua và điều kiện vẫn giữ nguyên).",
    ),
    (
        "Click to fill the goal from it.",
        "Bấm để điền mục tiêu từ CM này.",
    ),
    ("Round 1: {} ({})\n", "Vòng 1: {} ({})\n"),
    (
        "{} skills won't activate in it.\n",
        "{} skill sẽ không kích hoạt trong CM này.\n",
    ),
    (
        "Unlinked the {}; the course and conditions stay.",
        "Đã bỏ liên kết {}; đường đua và điều kiện vẫn giữ nguyên.",
    ),
    (
        "Filled the goal from the {}; {}.",
        "Đã điền mục tiêu từ {}; {}.",
    ),
    (
        "Filled course, distance, surface and conditions from the {} ({}).",
        "Đã điền đường đua, cự ly, mặt sân và điều kiện từ {} ({}).",
    ),
    (
        "{} isn't in Umalator's course list, so set distance and surface by hand",
        "{} không có trong danh sách đường đua của Umalator, nên hãy tự đặt cự ly và mặt sân",
    ),
    (
        "no race in races.json runs on {}, so pick the course by hand if you want one; Open Umalator still uses the preset's course",
        "không có race nào trong races.json chạy trên {}, nên tự chọn đường đua nếu bạn muốn; Open Umalator vẫn dùng đường đua của preset",
    ),
    ("Scenario", "Scenario"),
    ("\nLinked: {}. {}", "\nLiên kết: {}. {}"),
    (
        "\nPicked automatically: the scenario of the next Champions Meeting (uma.guide).",
        "\nTự chọn: scenario của Champions Meeting kế tiếp (uma.guide).",
    ),
    (
        "you own no linked cards",
        "bạn không sở hữu card liên kết nào",
    ),
    ("you own linked: {}", "bạn sở hữu card liên kết: {}"),
    ("Target", "Mục tiêu"),
    ("{} ({}), turns {}", "{} ({}), cua {}"),
    (
        "Clear the course (unlinks the CM too)",
        "Xóa đường đua (cũng bỏ liên kết CM)",
    ),
    ("{} (no race matches)", "{} (không có race khớp)"),
    ("🔍 search race or track", "🔍 tìm race hoặc track"),
    // Career → Plan: the meta sheet.
    (
        "📈 Pick an upcoming CM above to see what the meta sheet recommends for it. The deck fill and Auto-pick then put its picks first.",
        "📈 Chọn một CM sắp tới ở trên để xem sheet meta gợi ý gì cho CM đó. Điền deck và Auto-pick sau đó sẽ ưu tiên các gợi ý ấy.",
    ),
    ("ⓘ community PvP sheet", "ⓘ sheet PvP cộng đồng"),
    (
        "{}\n{}\nRead {} by tools/update_knowledge.py --only meta.\n{}",
        "{}\n{}\nĐọc ngày {} bởi tools/update_knowledge.py --only meta.\n{}",
    ),
    (
        "What the community's PvP sheet recommends for this CM. ✨ Recommend whole deck (Step 4) puts its picks (★) first; in Step 5, \"Meta sheet\" sets how much they count for skills. Opinion, not game data{}.",
        "Những gì sheet PvP của cộng đồng gợi ý cho CM này. ✨ Gợi ý cả deck (Bước 4) ưu tiên các lựa chọn của sheet (★); ở Bước 5, \"Sheet meta\" quyết định mức ảnh hưởng của chúng với skill. Ý kiến cộng đồng, không phải dữ liệu game{}.",
    ),
    (
        "; the sheet says its advice past CM 21 still needs checking",
        "; sheet cho biết gợi ý sau CM 21 vẫn cần kiểm chứng",
    ),
    ("Best styles", "Style tốt nhất"),
    ("ⓘ why", "ⓘ vì sao"),
    ("Target stats", "Chỉ số mục tiêu"),
    (
        "\"++\" means more is better.",
        "\"++\" nghĩa là càng cao càng tốt.",
    ),
    ("Use these", "Dùng các số này"),
    (
        "Set Step 3's target stats to these",
        "Đặt chỉ số mục tiêu ở Bước 3 thành các số này",
    ),
    ("Trainees", "Trainee"),
    (
        "You own her. Rated {} by the sheet. Click to make her the trainee.",
        "Bạn sở hữu cô ấy. Sheet đánh giá {}. Bấm để chọn làm trainee.",
    ),
    (
        "Not in your roster. Rated {} by the sheet.",
        "Không có trong roster. Sheet đánh giá {}.",
    ),
    (
        "The sheet's decks and skill advice",
        "Deck và gợi ý skill của sheet",
    ),
    ("{} deck", "Deck {}"),
    ("any {}", "bất kỳ {}"),
    ("Borrowed from a friend", "Mượn từ bạn bè"),
    ("One of your five", "Một trong năm card của bạn"),
    ("Brought for: {}", "Mang theo cho: {}"),
    ("Needs LB {} or higher{}", "Cần LB {} trở lên{}"),
    ("You own it at LB {}", "Bạn sở hữu ở LB {}"),
    ("Not in your roster", "Không có trong roster"),
    ("Also good", "Cũng tốt"),
    ("Honorable mention", "Gợi ý thêm"),
    ("Honorable mention: {}", "Gợi ý thêm: {}"),
    (
        "Green: you own it. ↔: the borrowed friend card.",
        "Xanh lá: bạn sở hữu. ↔: card mượn từ bạn bè.",
    ),
    ("Acceleration", "Tăng tốc"),
    ("Parents", "Parent"),
    (
        "The advice names these umas' uniques, which a trainee inherits from parents: {}.",
        "Gợi ý có nhắc unique của các uma này, thứ trainee thừa hưởng từ parent: {}.",
    ),
    // Career → Plan: the deck.
    ("Step 4 · Deck", "Bước 4 · Deck"),
    (
        "five of your cards plus one borrowed from a friend; click a slot to change it",
        "năm card của bạn cộng một card mượn từ bạn bè; bấm vào một ô để đổi",
    ),
    ("✨ Recommend whole deck", "✨ Gợi ý cả deck"),
    (
        "The meta sheet's cards for this CM and style come first (★), strongest backing first; simulated careers rank the rest.",
        "Card của sheet meta cho CM và style này được ưu tiên (★), mức ủng hộ mạnh nhất trước; career mô phỏng xếp hạng phần còn lại.",
    ),
    (
        "Replace all six slots with the best fits for this goal and trainee. {}",
        "Thay cả sáu ô bằng những card phù hợp nhất với mục tiêu và trainee này. {}",
    ),
    ("Recommended", "Đã gợi ý"),
    ("✨ Fill empty slots", "✨ Điền ô trống"),
    (
        "Keep the cards you chose and fill the rest. {}",
        "Giữ các card bạn đã chọn và điền phần còn lại. {}",
    ),
    ("Filled", "Đã điền"),
    ("Friend", "Bạn bè"),
    ("Slot {}", "Ô {}"),
    (
        "Empty: click to borrow a friend's SSR card",
        "Trống: bấm để mượn một card SSR của bạn bè",
    ),
    (
        "Empty: click to choose one of your cards",
        "Trống: bấm để chọn một card của bạn",
    ),
    ("Score {} · {}", "Điểm {} · {}"),
    (
        "Working out why this card fits…",
        "Đang tính lý do card này phù hợp…",
    ),
    ("Click to change", "Bấm để đổi"),
    ("Remove", "Xóa"),
    (
        "⚠ {} is the trainee; her card can't be in the deck.",
        "⚠ {} là trainee; card của cô ấy không thể nằm trong deck.",
    ),
    (
        "⚠ Two {} cards; a deck holds only one card per character.",
        "⚠ Có hai card {}; deck chỉ giữ một card mỗi nhân vật.",
    ),
    ("✨ Why these cards", "✨ Vì sao chọn các card này"),
    (
        "simulated on the target course",
        "mô phỏng trên đường đua mục tiêu",
    ),
    (
        "estimated from how their conditions fit (🏁 Simulate skills below to measure them)",
        "ước tính từ độ phù hợp điều kiện (bấm 🏁 Mô phỏng skill bên dưới để đo)",
    ),
    (
        "Each card is scored by what it adds to a simulated {} career with the rest of the deck: stats toward the target stats (less once a stat passes its target), skill points, and the scenario link. Then the skills it can teach, {}, for this trainee's style and the course's distance, direction and slopes. Scores are stat-point equivalents. Suggestions from a model (knowledge/scenarios.json lists its assumptions), not a solved optimum.",
        "Mỗi card được chấm theo phần nó đóng góp cho career {} mô phỏng cùng phần còn lại của deck: chỉ số hướng tới chỉ số mục tiêu (giảm khi một chỉ số vượt mục tiêu), điểm skill và liên kết scenario. Sau đó là các skill card có thể dạy, {}, cho style của trainee và cự ly, hướng cua, độ dốc của đường đua. Điểm quy đổi theo điểm chỉ số. Đây là gợi ý từ mô hình (knowledge/scenarios.json liệt kê giả định), không phải phương án tối ưu tuyệt đối.",
    ),
    ("Scoring the cards…", "Đang chấm điểm các card…"),
    ("(friend)", "(bạn bè)"),
    (
        "Recommendation score before the repeat-type penalty",
        "Điểm gợi ý trước khi trừ hình phạt trùng loại card",
    ),
    ("Choose a friend card", "Chọn card bạn bè"),
    ("Choose a card", "Chọn card"),
    ("✖ Close", "✖ Đóng"),
    ("Leave empty", "Để trống"),
    (
        "Any Global SSR, at the LB set under the slot. Find someone lending it:",
        "Bất kỳ SSR trên Global, ở LB đặt dưới ô. Tìm người cho mượn:",
    ),
    (
        "uma.moe: a friend database to search for a support card to borrow",
        "uma.moe: cơ sở dữ liệu bạn bè để tìm support card mượn",
    ),
    (
        "ChronoGenesis: search trainers' showcased umas and MLB support cards",
        "ChronoGenesis: tìm uma trưng bày và support card MLB của trainer khác",
    ),
    ("🔍 Search uma or title", "🔍 Tìm uma hoặc tên card"),
    ("All", "Tất cả"),
    (
        "Scoring every card for this slot (simulated careers)…",
        "Đang chấm mọi card cho ô này (career mô phỏng)…",
    ),
    ("✨ RECOMMENDED FOR THIS SLOT", "✨ GỢI Ý CHO Ô NÀY"),
    ("#{} · score {}\n{} [{}]\n{}", "#{} · điểm {}\n{} [{}]\n{}"),
    ("ALL CARDS", "MỌI CARD"),
    (
        "This is the trainee's own card",
        "Đây là card của chính trainee",
    ),
    (
        "Another card of this character is already in the deck",
        "Đã có card khác của nhân vật này trong deck",
    ),
    (
        "\n#{} for this slot · score {}\n{}",
        "\n#{} cho ô này · điểm {}\n{}",
    ),
    (
        "Deck totals show here once you pick cards.",
        "Tổng deck sẽ hiện ở đây khi bạn chọn card.",
    ),
    (" · friend LB{} Lv{}", " · bạn bè LB{} Lv{}"),
    ("{} cards{}", "{} card"),
    (
        "DECK TOTALS AT THESE LEVELS  ⓘ",
        "TỔNG DECK Ở CÁC CẤP NÀY  ⓘ",
    ),
    (
        "Sums of each card's effect at its level. Data, not a simulation.",
        "Tổng hiệu ứng của từng card ở cấp hiện tại. Dữ liệu, không phải mô phỏng.",
    ),
    ("Stat bonus / initial:  {}", "Bonus chỉ số / ban đầu:  {}"),
    ("Friendship", "Friendship"),
    ("Training", "Huấn luyện"),
    ("Mood", "Tâm trạng"),
    ("Specialty", "Chuyên môn"),
    ("Race bonus", "Bonus race"),
    ("Hint freq.", "Tần suất hint"),
    ("Hint levels", "Cấp hint"),
    ("Initial bond", "Bond ban đầu"),
    ("Fail protect", "Chống fail"),
    ("Energy cost", "Chi phí energy"),
    ("Skill points", "Điểm skill"),
    ("Wit recovery", "Hồi Wit"),
    (
        "💡 Skill hints from the deck: {} fit · {} anywhere · {} wrong",
        "💡 Skill hint từ deck: {} phù hợp · {} dùng được mọi nơi · {} sai",
    ),
    ("✔ Fits this target", "✔ Phù hợp mục tiêu này"),
    ("◯ Works anywhere", "◯ Dùng được ở mọi nơi"),
    (
        "✖ Wrong style, distance or surface",
        "✖ Sai style, cự ly hoặc mặt sân",
    ),
    ("innate", "bẩm sinh"),
    ("her events", "event của cô ấy"),
    ("{} link (lyrics event)", "{} link (event lyrics)"),
    // Career → Plan: the career model and the deck-fill messages.
    (
        "Per career on average: {} trainings ({} with a friendship/rainbow card), {} races.",
        "Trung bình mỗi career: {} buổi huấn luyện ({} có card friendship/rainbow), {} race.",
    ),
    (" (assumed)", " (giả định)"),
    (
        "Linked card: +{} SP, +{} stats ({}, assumed)",
        "Card liên kết: +{} SP, +{} chỉ số ({}, giả định)",
    ),
    ("Training values: {}", "Giá trị huấn luyện: {}"),
    (
        "Assumed: great mood, rest below {} energy, {} optional races, objective races won {}% of the time, chain events {}%, random events {}%.",
        "Giả định: mood tốt nhất, nghỉ khi energy dưới {}, {} race tùy chọn, race mục tiêu thắng {}% số lần, chain event {}%, random event {}%.",
    ),
    (
        "Rules from knowledge/scenarios.json (updated {}). Log real careers with their final SP in Results to check the model.",
        "Quy tắc từ knowledge/scenarios.json (cập nhật {}). Ghi career thật cùng SP cuối ở Kết quả để kiểm tra mô hình.",
    ),
    (
        "{} {}: {} ★ from the meta sheet's picks for {}, {} from simulated {} careers. Hover a card to see why.",
        "{} {}: {} ★ từ lựa chọn của sheet meta cho {}, {} từ career {} mô phỏng. Rê chuột lên card để xem lý do.",
    ),
    (
        "{} {} from simulated {} careers. Hover a card to see why; they're suggestions from a model.",
        "{} {} từ career {} mô phỏng. Rê chuột lên card để xem lý do; đây là gợi ý từ mô hình.",
    ),
    (" within the {} SP budget", " trong ngân sách {} SP"),
    (
        ", {} of them ★ recommended by the meta sheet for {}",
        ", {} trong đó ★ được sheet meta gợi ý cho {}",
    ),
    (
        "; nothing this deck teaches is on the meta sheet's list for {}",
        "; không có skill nào deck này dạy nằm trong danh sách của sheet meta cho {}",
    ),
    ("Picked {} skill{}{}{}.", "Đã chọn {} skill{}{}{}."),
    (
        "left out (Umalator doesn't know them): {}",
        "bỏ qua (Umalator không biết): {}",
    ),
    (
        "picked the {} course; check the layout",
        "đã chọn đường đua {}; kiểm tra layout",
    ),
    (
        "{} isn't in Umalator; pick the course there",
        "{} không có trong Umalator; hãy chọn đường đua ở đó",
    ),
    (
        "the {}'s course and conditions are estimated, not announced yet",
        "đường đua và điều kiện của {} là ước tính, chưa được công bố",
    ),
    (
        "conditions the plan leaves open come from {} ({})",
        "điều kiện kế hoạch để trống lấy từ {} ({})",
    ),
    // Career → Plan: Step 5, skills to buy.
    ("Step 5 · Skills to buy", "Bước 5 · Skill cần mua"),
    (
        "pick a trainee and cards to see the skills on offer",
        "chọn trainee và card để xem các skill có thể mua",
    ),
    ("{} chosen", "{} đã chọn"),
    (
        "What the chosen skills cost after discounts, against the SP budget",
        "Chi phí các skill đã chọn sau giảm giá, so với ngân sách SP",
    ),
    ("✖ Clear", "✖ Xóa"),
    ("Unpick every skill", "Bỏ chọn mọi skill"),
    ("Stop auto-pick", "Dừng auto-pick"),
    (
        "Choose the skills to buy within the SP budget. It picks at once from single gains, then races the candidates to find the set that gains the most together: each step adds the skill whose measured gain on top of the skills already chosen is best per SP, then it tries swaps and checks the set in 1000 races.",
        "Chọn skill cần mua trong ngân sách SP. Trước tiên chọn ngay theo gain riêng, sau đó cho các ứng viên đua để tìm bộ cộng dồn tốt nhất: mỗi bước thêm skill có gain đo được trên nền các skill đã chọn tốt nhất mỗi SP, rồi thử đổi chỗ và kiểm tra bộ trong 1000 race.",
    ),
    ("🏁 Simulate skills", "🏁 Mô phỏng skill"),
    (
        "Race each skill alone on the plan's course, in realistic races, for the Gain alone column and the card rankings",
        "Cho từng skill riêng lẻ đua trên đường đua của kế hoạch, trong race thực tế, để có cột Gain riêng và xếp hạng card",
    ),
    ("Stop simulating", "Dừng mô phỏng"),
    ("Aim", "Nhắm tới"),
    ("Meta sheet", "Sheet meta"),
    (
        "How much the community meta sheet's skill advice counts. Community opinion, not game data.",
        "Mức ảnh hưởng của gợi ý skill từ sheet meta cộng đồng. Ý kiến cộng đồng, không phải dữ liệu game.",
    ),
    ("Fast Learner (−10%)", "Fast Learner (−10%)"),
    (
        "The Fast Learner condition: every skill costs 10% of its base price less (uma.guide glossary)",
        "Điều kiện Fast Learner: mỗi skill giảm 10% giá gốc (từ điển uma.guide)",
    ),
    ("I'm at the skill shop now", "Tôi đang ở skill shop"),
    (
        "Use the career's real final stats and SP, and only the skills on offer with their hint levels",
        "Dùng chỉ số cuối và SP thật của career, chỉ những skill đang bán cùng cấp hint của chúng",
    ),
    ("Hide skills that don't fit", "Ẩn skill không phù hợp"),
    ("{} skills · {} SP", "{} skill · {} SP"),
    ("{} L in {} races", "{} L trong {} race"),
    (
        "The chosen set against the trainee with only her unique, in realistic races (wit checks and rushing on). Simulator output.",
        "Bộ skill đã chọn so với trainee chỉ có unique, trong race thực tế (bật kiểm tra Wit và rushing). Kết quả trình mô phỏng.",
    ),
    ("(95%: {} to {})", "(95%: {} đến {})"),
    (
        "their single gains add up to {} L",
        "tổng gain riêng của chúng là {} L",
    ),
    (
        "Each skill alone, added up. Skills overlap and fire in the same stretches, so together they gain less than the sum.",
        "Cộng gain của từng skill riêng lẻ. Các skill chồng lấn và kích hoạt cùng đoạn nên cộng lại sẽ ít hơn tổng.",
    ),
    (
        "Explain: how auto-pick chose",
        "Giải thích: cách auto-pick chọn",
    ),
    ("SP budget set by hand:", "Ngân sách SP đặt thủ công:"),
    (
        "SP budget ≈ {}: the mean of {} simulated {} careers with this deck (10–90%: {}–{}; {}).",
        "Ngân sách SP ≈ {}: trung bình của {} career {} mô phỏng với deck này (10–90%: {}–{}; {}).",
    ),
    (
        "No career simulation (knowledge/scenarios.json missing); set a budget by hand:",
        "Không có mô phỏng career (thiếu knowledge/scenarios.json); đặt ngân sách thủ công:",
    ),
    ("set by hand", "đặt thủ công"),
    ("At the skill shop", "Ở skill shop"),
    (
        "Enter the stats and SP the career ended with, then tick the skills on offer below and set their hint levels. Auto-pick then works on exactly those.",
        "Nhập chỉ số và SP khi career kết thúc, rồi tick các skill đang bán bên dưới và đặt cấp hint. Auto-pick sẽ chỉ làm việc trên đúng những skill đó.",
    ),
    ("📋 Paste the list", "📋 Dán danh sách"),
    (
        "Paste or type the skills on offer, one per line, with a hint level if any: \"Corner Adept ○ Lv3\"",
        "Dán hoặc gõ các skill đang bán, mỗi dòng một skill, kèm cấp hint nếu có: \"Corner Adept ○ Lv3\"",
    ),
    ("Untick all", "Bỏ chọn tất cả"),
    ("{} on offer", "{} đang bán"),
    (
        "One skill per line, e.g.\nCorner Adept ○ Lv3\nGroundwork 2",
        "Mỗi dòng một skill, vd.\nCorner Adept ○ Lv3\nGroundwork 2",
    ),
    ("Tick these", "Chọn các skill này"),
    (
        " {} line{} matched no skill.",
        " {} dòng{} không khớp skill nào.",
    ),
    (
        "Ticked {} skill{} as on offer.{}",
        "Đã chọn {} skill{} là đang bán.{}",
    ),
    (
        "You banned it (🚫): auto-pick never buys it.",
        "Bạn đã cấm (🚫): auto-pick không bao giờ mua skill này.",
    ),
    (
        "Not ticked as on offer at the shop.",
        "Chưa tick là đang bán ở shop.",
    ),
    (
        "Can't trigger on this course, by the simulator's check.",
        "Không thể kích hoạt trên đường đua này, theo bước kiểm tra của trình mô phỏng.",
    ),
    ("{} Pin it (📌) to buy it.", "{} Ghim (📌) để mua."),
    (
        "Only if a parent's spark passes it on (3–9% a time at 0 affinity, uma.guide). Pin it (📌) to count on it.",
        "Chỉ khi spark của parent truyền lại (3–9% mỗi lần ở 0 affinity, uma.guide). Ghim (📌) để tính đến nó.",
    ),
    (
        "The simulator doesn't have it (most are scenario skills). Pin it (📌) to buy it anyway.",
        "Trình mô phỏng không có skill này (phần lớn là skill scenario). Ghim (📌) để vẫn mua.",
    ),
    (
        "A debuff: it slows rivals, which one simulated uma can't show.",
        "Skill debuff: làm chậm đối thủ, điều một uma mô phỏng không thể hiện được.",
    ),
    (
        "It depends on where other umas are, which the simulator assumes always works out.",
        "Nó phụ thuộc vị trí của các uma khác, điều trình mô phỏng luôn giả định là thuận lợi.",
    ),
    ("{}'s spark{}", "spark của {}{}"),
    ("on offer at the shop", "đang bán ở shop"),
    (
        "Rating / parent needs each skill's evaluation points, which aren't in knowledge/ yet.",
        "Rating / parent cần điểm đánh giá của từng skill, hiện chưa có trong knowledge/.",
    ),
    (
        "No simulator data; run tools/update_knowledge.py --only umalator.",
        "Chưa có dữ liệu trình mô phỏng; hãy chạy tools/update_knowledge.py --only umalator.",
    ),
    (
        "Enter the final stats from the skill shop screen first.",
        "Hãy nhập chỉ số cuối từ màn hình skill shop trước.",
    ),
    (
        "Pick a course or Champions Meeting for the plan first.",
        "Hãy chọn đường đua hoặc Champions Meeting cho kế hoạch trước.",
    ),
    (
        "That course isn't in the simulator's data.",
        "Đường đua đó không có trong dữ liệu của trình mô phỏng.",
    ),
    ("Set an SP budget first.", "Hãy đặt ngân sách SP trước."),
    (
        "No skills to choose from: pick a trainee and cards, or tick what the shop offers.",
        "Không có skill để chọn: hãy chọn trainee và card, hoặc tick những gì shop bán.",
    ),
    (
        "Stopped. The quick pick stays.",
        "Đã dừng. Kết quả chọn nhanh vẫn giữ nguyên.",
    ),
    (
        "The plan changed while auto-pick ran; press ✨ Auto-pick again.",
        "Kế hoạch đã thay đổi trong khi auto-pick chạy; hãy bấm ✨ Auto-pick lần nữa.",
    ),
    (
        "Picked {} skill{} in {} s.",
        "Đã chọn {} skill{} trong {} s.",
    ),
    (
        "Nothing on offer gains enough on this course to be worth buying.",
        "Không skill nào đang bán tăng đủ trên đường đua này để đáng mua.",
    ),
    (
        " Pin skills from \"Not simulated\" below ({}) to spend it.",
        " Ghim skill từ mục \"Chưa mô phỏng\" bên dưới ({}) để dùng số SP này.",
    ),
    (
        " Nothing else on offer gains in simulation.",
        " Không skill nào khác đang bán tăng trong mô phỏng.",
    ),
    ("{} {} SP left.{}", "{} còn {} SP.{}"),
    (
        "Auto-pick stopped unexpectedly.",
        "Auto-pick dừng đột ngột.",
    ),
    (
        "Quick pick done. Now racing the skills to find the best set (about 20 seconds)…",
        "Đã chọn nhanh xong. Giờ cho các skill đua để tìm bộ tốt nhất (khoảng 20 giây)…",
    ),
    (
        " These come from single gains or estimates: {}",
        " Các kết quả này đến từ gain riêng hoặc ước tính: {}",
    ),
    (
        "Picked {} skill{} worth {} evaluation points for about {} SP (uma.guide's skill data; aptitude scaling of distance and style skills isn't modelled).",
        "Đã chọn {} skill{} trị giá {} điểm đánh giá với khoảng {} SP (dữ liệu skill của uma.guide; chưa mô hình hóa việc nhân aptitude cho skill cự ly và style).",
    ),
    ("Offer", "Bán"),
    ("Gain alone", "Gain riêng"),
    ("In this set", "Trong bộ này"),
    ("Cost", "Chi phí"),
    ("Triggers", "Kích hoạt"),
    ("Why", "Vì sao"),
    (
        "Bought: chosen skills first",
        "Đã mua: skill được chọn trước",
    ),
    (
        "📌 always buy · 🚫 never buy. Click to cycle.",
        "📌 luôn mua · 🚫 không bao giờ mua. Bấm để chuyển.",
    ),
    ("On offer at the shop", "Đang bán ở shop"),
    (
        "Its hint level at the shop: each level takes 10/20/30/35/40% off (uma.guide)",
        "Cấp hint ở shop: mỗi cấp giảm 10/20/30/35/40% (uma.guide)",
    ),
    (
        "Lengths it gains on its own on the plan's course, in realistic races (~ means an estimate). Simulator output.",
        "Số length skill tự tăng trên đường đua của kế hoạch, trong race thực tế (~ là ước tính). Kết quả trình mô phỏng.",
    ),
    (
        "What it adds to the chosen set: the set against the set without it. After auto-pick runs.",
        "Phần skill thêm vào bộ đã chọn: bộ có skill so với bộ không có skill đó. Sau khi auto-pick chạy.",
    ),
    (
        "SP after discounts, including the ○/white a ◎/gold needs",
        "SP sau giảm giá, gồm cả bản ○/white mà ◎/gold cần",
    ),
    ("Gain alone per 100 SP", "Gain riêng mỗi 100 SP"),
    (
        "How often it triggered in the 1000-race check",
        "Tần suất kích hoạt trong lần kiểm tra 1000 race",
    ),
    (
        "Why it was or wasn't bought",
        "Vì sao skill được mua hoặc không",
    ),
    (
        "Where the skill comes from: the trainee, a card, a parent's spark",
        "Nguồn của skill: trainee, card, spark của parent",
    ),
    ("Bought. Click to unpick.", "Đã mua. Bấm để bỏ chọn."),
    ("Click to buy", "Bấm để mua"),
    (
        "Pinned: auto-pick always buys it. Click to ban it instead.",
        "Đã ghim: auto-pick luôn mua. Bấm để cấm thay vào đó.",
    ),
    (
        "Banned: auto-pick never buys it. Click to clear.",
        "Đã cấm: auto-pick không bao giờ mua. Bấm để bỏ cấm.",
    ),
    (
        "Click to pin it (always buy); click again to ban it (never buy)",
        "Bấm để ghim (luôn mua); bấm lần nữa để cấm (không bao giờ mua)",
    ),
    ("One hint level lower", "Giảm một cấp hint"),
    (
        "The hint level the shop shows: each level takes 10% off (35% at Lv4, 40% at Lv5)",
        "Cấp hint shop hiển thị: mỗi cấp giảm 10% (35% ở Lv4, 40% ở Lv5)",
    ),
    ("One hint level higher", "Tăng một cấp hint"),
    ("the skill advice", "gợi ý skill"),
    ("a recommended card's note", "ghi chú của card được gợi ý"),
    (
        "a recommended gold's ○/white",
        "bản ○/white của gold được gợi ý",
    ),
    (
        "Recommended by the meta sheet ({}): {}. Community opinion, not game data.",
        "Được sheet meta gợi ý ({}): {}. Ý kiến cộng đồng, không phải dữ liệu game.",
    ),
    ("passive", "passive"),
    ("at gate", "tại gate"),
    (
        "{}: the simulator doesn't log it as triggering",
        "{}: trình mô phỏng không ghi nhận kích hoạt",
    ),
    ("{}\nFrom: {}", "{}\nTừ: {}"),
    (
        "Pinned (📌): always bought.",
        "Đã ghim (📌): luôn được mua.",
    ),
    (
        "Swapped in: the set gained {} L.",
        "Đổi vào: bộ skill tăng {} L.",
    ),
    (
        "Added at step {}: {} L on top of the skills before it{}.",
        "Thêm ở bước {}: {} L so với các skill trước đó{}.",
    ),
    (" (with {} first)", " (với {} trước)"),
    ("Chosen by you.", "Bạn tự chọn."),
    (
        "Chosen (quick pick, from single gains).",
        "Được chọn (chọn nhanh, từ gain riêng).",
    ),
    ("in simulation", "trong mô phỏng"),
    ("by estimate", "theo ước tính"),
    (
        "The meta sheet recommends it, but it's worth {} L for {} SP on this course {}.",
        "Sheet meta gợi ý skill này, nhưng nó chỉ đáng {} L với {} SP trên đường đua này {}.",
    ),
    (
        "Gains little on this course ({} L {}).",
        "Gain ít trên đường đua này ({} L {}).",
    ),
    (
        "Others gave more per SP once the set was chosen.",
        "Các skill khác cho nhiều hơn mỗi SP khi đã chọn bộ.",
    ),
    (
        "Not simulated ({}): debuffs and skills that depend on other umas",
        "Chưa mô phỏng ({}): debuff và skill phụ thuộc uma khác",
    ),
    (
        "One simulated uma can't value these, so auto-pick leaves them to you. Pin one to buy it.",
        "Một uma mô phỏng không định giá được các skill này, nên auto-pick để bạn tự quyết. Ghim một skill để mua nó.",
    ),
    (
        "Pin it: auto-pick always buys it",
        "Ghim: auto-pick luôn mua",
    ),
    (
        "The meta sheet: {}. Community opinion.",
        "Sheet meta: {}. Ý kiến cộng đồng.",
    ),
    (
        "{} {}: in the last spurt she runs about {}% slower than at A ({} m).",
        "{} {}: ở đoạn nước rút cuối cô ấy chạy chậm hơn khoảng {}% so với A ({} m).",
    ),
    (
        "{} {}: she accelerates about {}% slower than at A.",
        "{} {}: cô ấy tăng tốc chậm hơn khoảng {}% so với A.",
    ),
    (
        "{} {}: her Wit counts {}% less, so skills trigger less often and she rushes more.",
        "{} {}: Wit của cô ấy chỉ tính {}%, nên skill kích hoạt ít hơn và cô ấy bị rushes nhiều hơn.",
    ),
    ("⚠ Below B for this goal", "⚠ Dưới B cho mục tiêu này"),
    (
        "Pink sparks from parents can raise it ({}). ",
        "Spark hồng từ parent có thể nâng nó ({}). ",
    ),
    (
        "{}Or pick a trainee who fits better. From the race simulator's aptitude coefficients.",
        "{}Hoặc chọn trainee phù hợp hơn. Theo hệ số aptitude của trình mô phỏng đua.",
    ),
    // Career → Race sim: the plan's simulation.
    (
        "Simulated {} skills on {}.",
        "Đã mô phỏng {} skill trên {}.",
    ),
    (
        "The skill simulation stopped unexpectedly.",
        "Mô phỏng skill dừng đột ngột.",
    ),
    ("Save the plan first.", "Hãy lưu kế hoạch trước."),
    (
        "The plan file has no ## Simulation section.",
        "File kế hoạch không có mục ## Simulation.",
    ),
    ("Couldn't write {}: {}.", "Không ghi được {}: {}."),
    ("Wrote the results into {}.", "Đã ghi kết quả vào {}."),
    // Career → Plan: why a card was ranked where it was.
    ("no stats", "không có chỉ số"),
    ("career {} · SP {}", "career {} · SP {}"),
    (
        "Friendship {} · Training {}",
        "Friendship {} · Huấn luyện {}",
    ),
    (
        "skills ≈ {} L ({}; best: {} {} L)",
        "skill ≈ {} L ({}; tốt nhất: {} {} L)",
    ),
    ("estimated", "ước tính"),
    (" (+{} hint {} L)", " (+{} hint {} L)"),
    // Phase 6: Race sim, Results and charts.
    ("  {} recovers: {} at {} m", "  {} hồi phục: {} tại {} m"),
    (
        "  {}'s last spurt starts at {} m",
        "  spurt cuối của {} bắt đầu ở {} m",
    ),
    ("(this run)", "(lần này)"),
    (
        ", by {} L (≈ {} s) on average.",
        ", hơn {} L (≈ {} s) trung bình.",
    ),
    (", the final corner", ", góc cuối cùng"),
    (
        "95% interval for the average gap: {} to {} L (1 L = 2.5 m). Simulator output.",
        "Khoảng tin cậy 95% của khoảng cách trung bình: {} đến {} L (1 L = 2.5 m). Kết quả trình mô phỏng.",
    ),
    (
        "A Rust port of Moomoolator's engine (umalator.app, engine v2). It simulates the umas against a virtual pacemaker; order conditions count as met. The same seed gives the same results as the site.",
        "Bản port sang Rust của engine Moomoolator (umalator.app, engine v2). Trình này cho các uma đua với một pacemaker ảo; điều kiện thứ tự được tính là đã thỏa. Cùng seed cho cùng kết quả như trên trang web.",
    ),
    (
        "A formula instead of races: an instant, rougher stamina estimate.",
        "Dùng công thức thay cho đua: ước tính stamina tức thì, thô hơn.",
    ),
    ("A straight", "Một đường thẳng"),
    ("ALL GLOBAL SKILLS", "TẤT CẢ SKILL GLOBAL"),
    ("ALL GLOBAL TRAINEES", "TẤT CẢ TRAINEE GLOBAL"),
    ("Account", "Tài khoản"),
    (
        "Acts at the gate: a quicker start",
        "Kích hoạt ở cổng xuất phát: xuất phát nhanh hơn",
    ),
    (
        "Add each skill to Uma 1 in turn and measure how many lengths it gains on this course.",
        "Thêm lần lượt từng skill vào Uma 1 và đo số length nó tăng trên đường đua này.",
    ),
    ("Add to Uma 1", "Thêm vào Uma 1"),
    ("Add your first entry above.", "Thêm mục đầu tiên ở trên."),
    (
        "Added. Ask Claude to compare it with the plan and update insights.md.",
        "Đã thêm. Hãy nhờ Claude so sánh với kế hoạch và cập nhật insights.md.",
    ),
    ("Advanced options", "Tùy chọn nâng cao"),
    ("All Global skills", "Tất cả skill Global"),
    (
        "All charts share the course's distance axis. Speed and HP are the median race (the one whose gap is closest to the median).",
        "Mọi biểu đồ dùng chung trục cự ly của đường đua. Tốc độ và HP lấy từ race trung vị (race có khoảng cách gần trung vị nhất).",
    ),
    (
        "An instant estimate of Uma 1's HP needs on",
        "Ước tính nhanh nhu cầu HP của Uma 1 trên",
    ),
    ("Average gap", "Khoảng cách TB"),
    ("Back to the page you were on", "Quay lại trang bạn vừa xem"),
    ("Best for Uma 2", "Tốt nhất với Uma 2"),
    ("Both spurt", "Cả hai spurt"),
    (
        "Both umas see the same random events, so the gap shows the build difference",
        "Hai uma gặp cùng sự kiện ngẫu nhiên, nên khoảng cách thể hiện đúng khác biệt của build",
    ),
    ("Careers", "Career"),
    ("Champions Meeting", "Champions Meeting"),
    ("Claude's notes", "Ghi chú của Claude"),
    ("Click to remove", "Bấm để bỏ"),
    (
        "Compare this run's gaps with the current one",
        "So sánh khoảng cách của lần chạy này với lần hiện tại",
    ),
    (
        "Copied the summary as markdown.",
        "Đã sao chép tóm tắt dạng markdown.",
    ),
    (
        "Copy the setup and results as markdown, to paste into notes or a plan",
        "Sao chép thiết lập và kết quả dạng markdown, để dán vào ghi chú hoặc kế hoạch",
    ),
    (
        "Copy this build to Uma {}",
        "Sao chép build này sang Uma {}",
    ),
    ("Copy →", "Sao chép →"),
    ("Corner {}{} ({}–{} m)", "Góc {}{} ({}–{} m)"),
    ("Couldn't save {}", "Không lưu được {}"),
    ("Couldn't write: {}", "Không ghi được: {}"),
    ("Cumulative", "Tích lũy"),
    ("Downhill", "Xuống dốc"),
    (
        "Downhill mode saves about {} HP, so {} is needed",
        "Chế độ xuống dốc tiết kiệm khoảng {} HP, nên cần {}",
    ),
    ("Downhill saving", "Tiết kiệm khi xuống dốc"),
    (
        "Downhills: {} m; at this Wit, downhill mode is on {}% of the time on them.",
        "Đoạn dốc: {} m; với Wit này, chế độ xuống dốc bật {}% thời gian trên các đoạn đó.",
    ),
    ("Duel %", "Tỉ lệ đấu %"),
    ("Dueling", "Đấu tay đôi"),
    (
        "Each row is one skill: where along the course it activated across the races (darker, more often), its average start (the line) and how often it fired.",
        "Mỗi dòng là một skill: vị trí trên đường đua nơi nó kích hoạt qua các race (càng đậm càng thường xuyên), điểm bắt đầu trung bình (đường kẻ) và tần suất kích hoạt.",
    ),
    (
        "Each skill is added to Uma 1 in turn to see how much it helps on",
        "Lần lượt thêm từng skill vào Uma 1 để xem nó giúp ích bao nhiêu trên",
    ),
    ("Each uma", "Từng uma"),
    ("Earlier Champions Meetings…", "Champions Meeting trước đó…"),
    (
        "Earlier runs to compare against, each uma's numbers, and export",
        "Các lần chạy trước để so sánh, số liệu của từng uma và xuất dữ liệu",
    ),
    (
        "Enough for a full spurt, with {} HP to spare. Minimum Stamina for it: {}.",
        "Đủ cho full spurt, còn dư {} HP. Stamina tối thiểu để đạt: {}.",
    ),
    ("Entries ({})", "Mục ({})"),
    (
        "Entries are saved in the results folder, newest first. Claude reads them to check plans against what really happened.",
        "Các mục được lưu trong thư mục results, mới nhất trước. Claude đọc chúng để đối chiếu kế hoạch với những gì thực sự đã xảy ra.",
    ),
    (
        "Every race's gap at the finish. Right of zero, Uma 2 won.",
        "Khoảng cách mỗi race khi về đích. Bên phải số 0 là Uma 2 thắng.",
    ),
    ("Export", "Xuất dữ liệu"),
    ("Files", "Tệp"),
    ("Fill in more than the date.", "Hãy điền thêm ngoài ngày."),
    (
        "Fill in the Planner's trainee, target stats, skills, course and conditions",
        "Điền trainee, chỉ số mục tiêu, skill, đường đua và điều kiện từ Kế hoạch",
    ),
    (
        "Fill in what happened and press ＋ Add entry. The ✨ buttons suggest values from your roster and the open plan. Saved to {}.",
        "Điền những gì đã xảy ra rồi nhấn ＋ Thêm mục. Các nút ✨ gợi ý giá trị từ roster và kế hoạch đang mở. Lưu vào {}.",
    ),
    ("Final", "Cuối"),
    ("Final leg", "Chặng cuối"),
    ("Force a full spurt", "Ép full spurt"),
    (
        "Front runners fighting for the lead early on",
        "Các uma chạy trước tranh vị trí dẫn đầu ngay từ đầu",
    ),
    ("Full spurt", "Full spurt"),
    ("Gap per race", "Khoảng cách mỗi race"),
    ("Ground", "Mặt sân"),
    (
        "HP at the finish and what a full spurt needs",
        "HP khi về đích và full spurt cần những gì",
    ),
    ("HP calculator", "Máy tính HP"),
    ("HP left", "HP còn lại"),
    (
        "HP left at the finish, per race",
        "HP còn lại khi về đích, theo từng race",
    ),
    ("HP survival", "Sống sót (HP)"),
    (
        "HP: shaded, rushed; +, a recovery skill",
        "HP: vùng tô là rushed; dấu +, skill hồi phục",
    ),
    (
        "Her {} aptitude for this race, S (best) to G",
        "Aptitude {} của uma cho đua này, từ S (tốt nhất) đến G",
    ),
    (
        "Hints and events from cards you own, skills of umas you own, and Uma 1's own skills",
        "Hint và sự kiện từ card bạn sở hữu, skill của uma bạn sở hữu, và skill riêng của Uma 1",
    ),
    ("Histogram", "Biểu đồ cột"),
    (
        "How many races ended with each gap",
        "Có bao nhiêu race kết thúc ở mỗi khoảng cách",
    ),
    (
        "How many races to run. More is steadier but slower; 1000 is plenty.",
        "Số race cần chạy. Nhiều hơn thì ổn định hơn nhưng chậm hơn; 1000 là đủ.",
    ),
    ("In the main UmaYu folder", "Trong thư mục UmaYu chính"),
    (
        "Its conditions can't be met on this course",
        "Điều kiện của nó không thể thỏa trên đường đua này",
    ),
    ("Knowledge", "Kiến thức"),
    ("L / 100 SP", "L / 100 SP"),
    ("L, Uma 2's lead", "L, Uma 2 dẫn trước"),
    ("Lane movement", "Đổi làn"),
    ("Last spurt", "Spurt cuối"),
    (
        "Late-race duels on the final straight",
        "Đấu tay đôi cuối đường đua trên đường thẳng về đích",
    ),
    ("League of Heroes", "League of Heroes"),
    ("Left out: {}", "Đã bỏ qua: {}"),
    (
        "Level of her unique skill, 1–6",
        "Cấp skill unique của cô ấy, 1–6",
    ),
    ("Logs", "Sổ ghi"),
    (
        "Markdown notes in this folder. Click one to read it.",
        "Ghi chú markdown trong thư mục này. Bấm một file để đọc.",
    ),
    ("Max HP", "Max HP"),
    (
        "Max HP and what a full-speed race needs (HP calculator)",
        "HP tối đa và nhu cầu của một race full tốc độ (máy tính HP)",
    ),
    (
        "Max HP {}; a full spurt needs about {} ({} before the final leg, {} after).",
        "HP tối đa {}; full spurt cần khoảng {} ({} trước chặng cuối, {} sau đó).",
    ),
    ("Mean", "Trung bình"),
    ("Median", "Trung vị"),
    ("Middle", "Giữa"),
    ("Middle leg", "Chặng giữa"),
    ("Min Stamina", "Stamina tối thiểu"),
    ("Min Stamina (downhills)", "Stamina tối thiểu (có dốc)"),
    ("Min–max", "Min–max"),
    (
        "Moomoolator's closed-form estimate from Uma 1's Stamina, Guts, Wit and Speed. It ignores mood, aptitude and ground; confirm with ▶ Stamina.",
        "Ước tính công thức của Moomoolator từ Stamina, Guts, Wit và Speed của Uma 1. Bỏ qua tâm trạng, aptitude và mặt sân; hãy kiểm tra lại bằng ▶ Stamina.",
    ),
    ("Needed", "Cần"),
    ("Never activated", "Chưa từng kích hoạt"),
    (
        "Never activated in these races.",
        "Chưa từng kích hoạt trong các race này.",
    ),
    (
        "Never fired: failed Wit checks, or a rare condition",
        "Chưa kích hoạt: trượt kiểm tra Wit, hoặc điều kiện hiếm",
    ),
    (
        "Never fired: its condition didn't come up",
        "Chưa kích hoạt: điều kiện không xảy ra",
    ),
    ("New seed each run", "Seed mới mỗi lần chạy"),
    ("No outfit (no unique)", "Không có outfit (không có unique)"),
    (
        "No skills to test for this uma on this course.",
        "Không có skill nào để thử cho uma này trên đường đua này.",
    ),
    (
        "No table found in the file.",
        "Không tìm thấy bảng trong file.",
    ),
    (
        "No table found in this file.",
        "Không tìm thấy bảng trong file này.",
    ),
    ("Nothing logged yet", "Chưa ghi gì"),
    (
        "ON YOUR ACCOUNT (cards and umas you own)",
        "TRÊN TÀI KHOẢN CỦA BẠN (card và uma bạn sở hữu)",
    ),
    (
        "Off: rerun with the same seed for repeatable results",
        "Tắt: chạy lại với cùng seed để có kết quả lặp lại",
    ),
    (
        "One log per kind of result. Pick one to add to it or read it.",
        "Mỗi loại kết quả có một sổ. Chọn một sổ để thêm mục hoặc đọc.",
    ),
    (
        "Open it in your default program for .md files",
        "Mở bằng chương trình mặc định cho file .md",
    ),
    ("Opening", "Mở đầu"),
    (
        "Opening and middle legs {} + final leg and spurt {}",
        "Chặng đầu và giữa {} + chặng cuối và spurt {}",
    ),
    ("Opening leg", "Chặng mở đầu"),
    ("Overview", "Tổng quan"),
    (
        "Passive: on for the whole race while its condition holds",
        "Passive: có hiệu lực suốt đua khi điều kiện được thỏa",
    ),
    (
        "Pick a Champions Meeting on the left: the meta sheet recommends stats per CM.",
        "Chọn một Champions Meeting ở bên trái: sheet meta khuyến nghị chỉ số theo từng CM.",
    ),
    ("Pick a course first.", "Hãy chọn đường đua trước."),
    ("Pinned: {}", "Đã ghim: {}"),
    ("Plans", "Kế hoạch"),
    (
        "Press the button to run it in the background.",
        "Nhấn nút để chạy nền.",
    ),
    ("Race", "Đua"),
    (
        "Race Uma 1 against Uma 2 many times: who wins, by how much, and where.",
        "Cho Uma 1 đua với Uma 2 nhiều lần: ai thắng, thắng bao nhiêu và ở đâu.",
    ),
    ("Race setup", "Thiết lập đua"),
    ("Read the file again from disk", "Đọc lại file từ ổ đĩa"),
    ("Recent runs", "Các lần chạy gần đây"),
    (
        "Recovery skills (% of max HP)",
        "Skill hồi phục (% HP tối đa)",
    ),
    (
        "Remove every skill except the unique",
        "Bỏ mọi skill trừ unique",
    ),
    (
        "Run Uma 1 alone and see the HP she has left at the finish.",
        "Cho Uma 1 chạy một mình và xem HP còn lại khi về đích.",
    ),
    ("Runs", "Lần chạy"),
    (
        "Runs in the background on the last run's setup",
        "Chạy nền với thiết lập của lần chạy trước",
    ),
    ("SP", "SP"),
    (
        "Save the results shown on the tab you pick next (Overview, Race…) as a picture in Pictures\\UmaYu",
        "Lưu kết quả đang hiển thị ở tab bạn chọn tiếp theo (Tổng quan, Đua…) thành ảnh trong Pictures\\UmaYu",
    ),
    ("Saved at {}", "Lưu tại {}"),
    ("Saved {}", "Đã lưu {}"),
    ("Scrub through the race", "Kéo để tua race"),
    ("Season", "Mùa"),
    (
        "Set up Uma 1 and Uma 2 and press ▶ Run. The gap is Uma 2's lead over Uma 1 in lengths (1 L = 2.5 m).",
        "Thiết lập Uma 1 và Uma 2 rồi nhấn ▶ Chạy. Khoảng cách là số length Uma 2 dẫn trước Uma 1 (1 L = 2.5 m).",
    ),
    (
        "Short of a full spurt: needs {} Stamina, or recovery skills worth {}% of max HP.",
        "Chưa đủ full spurt: cần {} Stamina, hoặc skill hồi phục tương đương {}% HP tối đa.",
    ),
    (
        "Shows how far short of a full spurt the build is",
        "Cho thấy build còn thiếu bao nhiêu so với full spurt",
    ),
    ("Skills ({})", "Skill ({})"),
    (
        "Skills can fail to activate on a Wit roll",
        "Skill có thể không kích hoạt khi roll Wit",
    ),
    ("Skills on your account", "Skill trên tài khoản của bạn"),
    (
        "Sort by lengths per 100 SP",
        "Sắp xếp theo length trên 100 SP",
    ),
    ("Sparks", "Spark"),
    (
        "Speed (m/s): bars above are skills firing; dashes, the last spurt; ✕, HP ran out",
        "Tốc độ (m/s): thanh phía trên là skill kích hoạt; nét đứt là spurt cuối; ✕ là hết HP",
    ),
    ("Speed and HP, median run", "Tốc độ và HP, race trung vị"),
    ("Spot struggle", "Tranh vị trí"),
    ("Spurt", "Spurt"),
    (
        "Start the simulation. It runs in the background; you can keep using the app.",
        "Bắt đầu mô phỏng. Mô phỏng chạy nền, bạn vẫn dùng app bình thường.",
    ),
    ("Stat templates", "Mẫu chỉ số"),
    ("Stop", "Dừng"),
    ("Stop the run", "Dừng lần chạy này"),
    ("Style apt.", "Apt. phong cách"),
    ("Surplus", "Dư"),
    ("Swap Uma 1 and Uma 2", "Đổi chỗ Uma 1 và Uma 2"),
    ("Sync RNG", "Đồng bộ RNG"),
    ("THIS UMA'S SKILLS", "SKILL CỦA UMA NÀY"),
    ("Team Trials", "Team Trials"),
    ("Test", "Thử"),
    (
        "The HP calculator ignores mood, aptitude and ground; the races above don't.",
        "Máy tính HP bỏ qua tâm trạng, aptitude và mặt sân; các race phía trên thì không.",
    ),
    (
        "The community meta sheet's recommended stats for {}: {}. Community opinion, not game data.",
        "Chỉ số sheet meta cộng đồng khuyến nghị cho {}: {}. Ý kiến cộng đồng, không phải dữ liệu game.",
    ),
    (
        "The course, where the lengths are won, and speed and HP along the race",
        "Đường đua, length được thắng ở đâu, cùng tốc độ và HP suốt cuộc đua",
    ),
    (
        "The defaults match umalator.app. Leave them unless you know you want something else.",
        "Mặc định khớp với umalator.app. Cứ giữ nguyên trừ khi bạn muốn khác.",
    ),
    ("The final straight", "Đường thẳng cuối cùng"),
    (
        "The gap is within noise: treat the two builds as even, or run more races.",
        "Khoảng cách nằm trong sai số: coi hai build ngang nhau, hoặc chạy thêm race.",
    ),
    (
        "The last 10 runs this session. Pin one to draw its gaps as an outline on Overview's chart.",
        "10 lần chạy gần nhất trong phiên này. Ghim một lần để vẽ khoảng cách của nó thành viền trên biểu đồ Tổng quan.",
    ),
    ("The race", "Cuộc đua"),
    (
        "The share of races that ended below each gap",
        "Tỉ lệ race kết thúc dưới mỗi khoảng cách",
    ),
    (
        "The simulation stopped unexpectedly.",
        "Mô phỏng dừng đột ngột.",
    ),
    (
        "The simulator can't run this skill",
        "Trình mô phỏng không chạy được skill này",
    ),
    (
        "The starting point for the random numbers. The same seed gives the same results.",
        "Điểm khởi đầu của các số ngẫu nhiên. Cùng seed cho cùng kết quả.",
    ),
    ("Time", "Thời gian"),
    (
        "Took out {} skills that won't activate in the {}.",
        "Đã bỏ {} skill sẽ không kích hoạt trong {}.",
    ),
    ("Turned off in the {}", "Đã tắt trong {}"),
    (
        "Two umas race each other many times on",
        "Hai uma đua với nhau nhiều lần trên",
    ),
    ("Typical gap", "Khoảng cách điển hình"),
    ("Uma 1 ahead", "Uma 1 dẫn"),
    ("Uma 1 by {} L ({}%)", "Uma 1 hơn {} L ({}%)"),
    (
        "Uma 1 is the plan's trainee without her bought skills; Uma 2 has them.",
        "Uma 1 là trainee của kế hoạch chưa có skill đã mua; Uma 2 có các skill đó.",
    ),
    (
        "Uma 1 races alone to check her HP lasts on",
        "Uma 1 chạy một mình để kiểm tra HP có đủ trên",
    ),
    ("Uma 1 spurts", "Uma 1 spurt"),
    ("Uma 1 wins {}%", "Uma 1 thắng {}%"),
    (
        "Uma 2 against Uma 2 without each skill in turn, 200 races each: the lengths she'd lose without it.",
        "Uma 2 đua với chính mình khi thiếu lần lượt từng skill, mỗi lượt 200 race: số length cô ấy sẽ mất nếu thiếu skill đó.",
    ),
    ("Uma 2 ahead", "Uma 2 dẫn"),
    ("Uma 2 by {} L ({}%)", "Uma 2 hơn {} L ({}%)"),
    ("Uma 2 spurts", "Uma 2 spurt"),
    ("Uma 2 wins {}%", "Uma 2 thắng {}%"),
    (
        "Uma 2 with +50 in each stat in turn, 300 races each: the lengths each +50 gains.",
        "Uma 2 cộng lần lượt +50 vào từng chỉ số, mỗi lượt 300 race: số length mỗi +50 mang lại.",
    ),
    ("Uma 2's lead (L)", "Uma 2 dẫn trước (L)"),
    (
        "Uma 2's lead along the race: the line is the median race, the band holds 8 races in 10.",
        "Uma 2 dẫn trước dọc đường đua: đường là race trung vị, dải chứa 8 trên 10 race.",
    ),
    ("Uma 2's lead over Uma 1", "Uma 2 dẫn trước Uma 1"),
    ("Uma {}'s stats", "Chỉ số của Uma {}"),
    (
        "Umalator's default (1200 / 1200 / 800 / 400 / 400)",
        "Mặc định của Umalator (1200 / 1200 / 800 / 400 / 400)",
    ),
    (
        "Umas can get over-eager mid-race and burn HP, as in the game",
        "Uma có thể hăng quá giữa đường đua và tốn HP, như trong game",
    ),
    (
        "Umas move between lanes on corners and straights",
        "Uma đổi làn ở các góc cua và đường thẳng",
    ),
    ("Unique Lv", "Cấp unique"),
    ("Uphill", "Lên dốc"),
    (
        "Upper bound: the chart assumes the skill's conditions are met as modelled (order conditions always). Uniques and evolved skills are left out.",
        "Cận trên: biểu đồ giả định điều kiện của skill luôn thỏa như mô hình (điều kiện thứ tự luôn đúng). Skill unique và skill đã tiến hóa bị loại.",
    ),
    ("WHAT TO TEST", "NỘI DUNG CẦN THỬ"),
    (
        "Watch the median race: two dots run the course, and skills pop up as they fire",
        "Xem race trung vị: hai chấm chạy trên đường đua, skill hiện lên khi kích hoạt",
    ),
    ("Weather", "Thời tiết"),
    (
        "What each of Uma 2's skills is worth",
        "Mỗi skill của Uma 2 đáng giá bao nhiêu",
    ),
    (
        "Where Uma 2's next training points matter",
        "Điểm huấn luyện tiếp theo của Uma 2 quan trọng ở đâu",
    ),
    ("Where each skill fired", "Skill kích hoạt ở đâu"),
    (
        "Where each skill fired, what each is worth, and where training points matter",
        "Skill kích hoạt ở đâu, mỗi skill đáng giá bao nhiêu và điểm huấn luyện quan trọng ở đâu",
    ),
    ("Where the lengths are won", "Length được thắng ở đâu"),
    (
        "Who wins, by how much, and how the gaps spread",
        "Ai thắng, thắng bao nhiêu và khoảng cách trải ra thế nào",
    ),
    ("With downhills", "Có dốc"),
    ("Worst for Uma 2", "Tệ nhất với Uma 2"),
    (
        "Writes these results into the plan's ## Simulation section",
        "Ghi kết quả này vào mục ## Simulation của kế hoạch",
    ),
    ("YOUR UMAS", "UMA CỦA BẠN"),
    (
        "Your plan's targets ({})",
        "Chỉ số mục tiêu của kế hoạch ({})",
    ),
    ("\nPinned run: {} runs here", "\nLần đã ghim: {} race ở đây"),
    ("\nScenario: {}", "\nKịch bản: {}"),
    (
        "\n{} skills won't activate; picking it drops them from both builds.",
        "\n{} skill sẽ không kích hoạt; chọn nó sẽ bỏ chúng khỏi cả hai build.",
    ),
    ("announced", "đã công bố"),
    ("finished with HP left {}%", "về đích còn {}% HP"),
    ("from Stamina", "từ Stamina"),
    ("largest gap", "khoảng cách lớn nhất"),
    ("left", "trái"),
    ("max", "tối đa"),
    ("mean", "trung bình"),
    ("median", "trung vị"),
    (
        "median HP saved on slopes",
        "trung vị HP tiết kiệm trên dốc",
    ),
    ("median of {} runs", "trung vị của {} race"),
    (
        "median; below 0 means she ran dry",
        "trung vị; dưới 0 nghĩa là đã cạn HP",
    ),
    ("needs", "cần"),
    ("no (+{}% heal)", "không (+{}% hồi)"),
    ("no outfit", "không có outfit"),
    (
        "of runs at top speed to the end",
        "số race chạy hết tốc độ tới đích",
    ),
    ("of runs finished with HP left", "số race về đích còn HP"),
    ("pinned run (outline)", "lần chạy đã ghim (viền)"),
    ("rate", "tỉ lệ"),
    ("right", "phải"),
    ("runs", "race"),
    ("share", "tỉ lệ"),
    ("smallest gap", "khoảng cách nhỏ nhất"),
    ("straight", "thẳng"),
    (
        "the simulator applies it without logging it as firing.",
        "trình mô phỏng áp dụng mà không ghi nhận là đã kích hoạt.",
    ),
    ("yes", "có"),
    ("{} ({}–{} m)", "{} ({}–{} m)"),
    (
        "{} m: {} by {} L (median)\nIn 8 of 10 races: {} to {} L",
        "{} m: {}, {} L (trung vị)\nTrong 8 trên 10 race: {} đến {} L",
    ),
    ("{} runs", "{} race"),
    (
        "{} to {} L: {} runs\n{}% of runs end below {} L",
        "{} đến {} L: {} race\n{}% số race kết thúc dưới {} L",
    ),
    (
        "{} vs {} · {} · {} · {} races",
        "{} vs {} · {} · {} · {} race",
    ),
    ("{} wins {}% of {} races", "{} thắng {}% trong {} race"),
    ("{} {}% ({}–{} m)", "{} {}% ({}–{} m)"),
    (
        "{}\nActivated in {}% of races, on average at {} m.\nMost often {}–{} m.",
        "{}\nKích hoạt trong {}% race, trung bình ở {} m.\nThường nhất ở {}–{} m.",
    ),
    ("{}–{} {}: {} runs", "{}–{} {}: {} race"),
    ("← Back", "← Quay lại"),
    ("← Copy", "← Sao chép"),
    ("↗ Open in editor", "↗ Mở bằng trình soạn thảo"),
    ("↻ Run again", "↻ Chạy lại"),
    ("⇄ Swap", "⇄ Đổi chỗ"),
    ("⏸ Pause", "⏸ Tạm dừng"),
    ("▶ Replay", "▶ Xem lại"),
    ("▶ Resume", "▶ Tiếp tục"),
    ("▶ Run", "▶ Chạy"),
    (
        "▶ Run simulates Uma 1 alone and reports the HP she has left at the finish.",
        "▶ Chạy sẽ mô phỏng Uma 1 một mình và báo HP còn lại khi về đích.",
    ),
    (
        "▶ Run tests every skill on Uma 1, two rounds like Moomoolator (25 runs, then 175 more for skills that gain). Wit checks, rushing and lane movement are off so each skill's own effect shows.",
        "▶ Chạy sẽ thử mọi skill trên Uma 1, hai vòng như Moomoolator (25 race, rồi thêm 175 race cho skill có gain). Kiểm tra Wit, rushing và đổi làn đều tắt để thấy đúng hiệu ứng riêng của từng skill.",
    ),
    ("⚖ Compare", "⚖ So sánh"),
    ("⚡ Quick", "⚡ Nhanh"),
    ("✨ pick", "✨ chọn"),
    ("＋ Add entry", "＋ Thêm mục"),
    ("＋ Add skill…", "＋ Thêm skill…"),
    ("＋ New entry", "＋ Mục mới"),
    ("＋ add", "＋ thêm"),
    ("🏁 Race sim", "🏁 Mô phỏng đua"),
    ("💾 Save to plan", "💾 Lưu vào kế hoạch"),
    ("📈 Use meta stats", "📈 Dùng chỉ số meta"),
    ("📊 Skills", "📊 Skill"),
    ("📋 Copy summary", "📋 Sao chép tóm tắt"),
    ("📋 Fill from the open plan", "📋 Điền từ kế hoạch đang mở"),
    ("📋 Load from Planner", "📋 Tải từ Kế hoạch"),
    ("📌 Pin", "📌 Ghim"),
    ("📌 Pinned", "📌 Đã ghim"),
    ("🔄 Reload", "🔄 Tải lại"),
    ("🖼 Save chart as PNG", "🖼 Lưu biểu đồ thành PNG"),
    ("First into the late race", "Vào chặng cuối đầu tiên"),
    ("Finish (game clock)", "Về đích (đồng hồ game)"),
    ("HP runs out (when it does)", "HP cạn (khi xảy ra)"),
    ("Last spurt starts", "Spurt cuối bắt đầu"),
    ("Spurt speed lost when short", "Tốc độ spurt mất khi thiếu"),
    ("Start delay", "Trễ xuất phát"),
    ("Date", "Ngày"),
    ("Purpose", "Mục đích"),
    ("Deck (LB)", "Deck (LB)"),
    (
        "Speed / Stamina / Power / Guts / Wit",
        "Speed / Stamina / Power / Guts / Wit",
    ),
    ("Final aptitudes", "Aptitude cuối"),
    ("Key skills", "Skill chính"),
    ("Total SP (spent + left)", "Tổng SP (đã tiêu + còn lại)"),
    ("Rank / score", "Hạng / điểm"),
    ("Cup", "Cup"),
    ("Team", "Đội"),
    ("Round", "Vòng"),
    ("Wins / races", "Thắng / race"),
    ("Date / season", "Ngày / mùa"),
    ("League / rank", "Giải / hạng"),
    ("Target sparks", "Spark mục tiêu"),
    ("Blue", "Xanh dương"),
    ("Pink", "Hồng"),
    ("Green", "Xanh lá"),
    ("White", "Trắng"),
    ("Parents used", "Parent đã dùng"),
    ("Final stats", "Chỉ số cuối"),
    ("Hit?", "Trúng?"),
    ("Date / week", "Ngày / tuần"),
    ("Class", "Hạng"),
    ("Umas on the team", "Uma trong đội"),
    ("Placings", "Thứ hạng"),
    ("Score", "Điểm"),
    ("Opponent strength", "Sức mạnh đối thủ"),
    // Phase 7: Ask, setup and profiles.
    (
        "Waiting for your answer on the card above.",
        "Đang chờ bạn trả lời trên thẻ ở trên.",
    ),
    ("Thinking…", "Đang suy nghĩ…"),
    ("Done: {}", "Xong: {}"),
    ("Looking up: {}", "Đang tra: {}"),
    ("Working…", "Đang xử lý…"),
    ("Preparing: {}", "Đang chuẩn bị: {}"),
    (
        "The model declined this request.",
        "Model đã từ chối yêu cầu này.",
    ),
    ("Declined: {}", "Bị từ chối: {}"),
    (
        "The answer hit the length limit and is cut off.",
        "Câu trả lời chạm giới hạn độ dài và bị cắt.",
    ),
    ("Answered by {} (fallback).", "Trả lời bởi {} (dự phòng)."),
    (
        "Its input was cut off or not valid JSON; the model was told to try again.",
        "Đầu vào của nó bị cắt hoặc không phải JSON hợp lệ; model đã được yêu cầu thử lại.",
    ),
    (
        "Stopped after many steps. Ask it to carry on, or ask something narrower.",
        "Đã dừng sau nhiều bước. Hãy bảo nó tiếp tục, hoặc hỏi phạm vi hẹp hơn.",
    ),
    ("Stopped.", "Đã dừng."),
    (
        "OpenCode isn't installed: tools/opencode/opencode.exe is missing.",
        "OpenCode chưa được cài: thiếu tools/opencode/opencode.exe.",
    ),
    ("The assistant saved '{}'", "Trợ lý đã lưu '{}'"),
    ("{} (allowed in Settings)", "{} (đã cho phép trong Cài đặt)"),
    ("no trainee yet", "chưa có trainee"),
    (
        "{} · {} · {} cards · {} skills",
        "{} · {} · {} card · {} skill",
    ),
    ("The assistant's working plan", "Kế hoạch nháp của trợ lý"),
    ("OpenCode isn't installed yet", "OpenCode chưa được cài"),
    (
        "It answers your questions for free, with no key and no account.",
        "Nó trả lời câu hỏi miễn phí, không cần key hay tài khoản.",
    ),
    ("Downloading… {} / {} MB", "Đang tải… {} / {} MB"),
    ("⬇ Download OpenCode ({} MB)", "⬇ Tải OpenCode ({} MB)"),
    (
        "Official release {} from github.com/anomalyco/opencode. The app checks its SHA-256 against the published one before installing it to tools\\opencode.",
        "Bản phát hành chính thức {} từ github.com/anomalyco/opencode. App kiểm tra SHA-256 với bản đã công bố trước khi cài vào tools\\opencode.",
    ),
    (
        "OpenCode is installed. Ask away.",
        "Đã cài OpenCode. Hỏi thoải mái.",
    ),
    (
        "Couldn't list OpenCode models: {}",
        "Không liệt kê được model OpenCode: {}",
    ),
    ("OpenCode: {}", "OpenCode: {}"),
    (
        "OpenCode finished without an answer. Try again or pick another model.",
        "OpenCode kết thúc mà không có câu trả lời. Hãy thử lại hoặc chọn model khác.",
    ),
    ("Not answered in time.", "Không được trả lời kịp."),
    (
        "Ask about your account, or ask it to plan and simulate a career…  (Enter to send, Shift+Enter for a new line)",
        "Hỏi về tài khoản của bạn, hoặc bảo nó lập kế hoạch và mô phỏng một career…  (Enter để gửi, Shift+Enter để xuống dòng)",
    ),
    ("■ Stop", "■ Dừng"),
    (
        "Stop the answer and any simulation it started",
        "Dừng câu trả lời và mọi mô phỏng nó đã bắt đầu",
    ),
    ("Send ➤", "Gửi ➤"),
    ("💬 Ask", "💬 Hỏi đáp"),
    (
        "Ask in plain English, like you'd ask a friend who knows the game. It answers from your roster and the game data, and can plan and simulate careers for you.",
        "Hỏi bằng ngôn ngữ tự nhiên, như hỏi một người bạn hiểu game. Nó trả lời từ roster và dữ liệu game của bạn, đồng thời có thể lập kế hoạch và mô phỏng career giúp bạn.",
    ),
    ("What do you want to know?", "Bạn muốn biết gì?"),
    (
        "A free model (via OpenCode)",
        "Model miễn phí (qua OpenCode)",
    ),
    (
        "{} looks things up in your roster, the uma.guide data, your plans and results, labels what is data and what is opinion, and can build a plan and race it in the simulator. You can undo every change it makes.",
        "{} tra cứu roster, dữ liệu uma.guide, kế hoạch và kết quả của bạn, ghi rõ đâu là dữ liệu đâu là ý kiến, đồng thời có thể lập kế hoạch và cho đua trong trình mô phỏng. Bạn có thể hoàn tác mọi thay đổi nó thực hiện.",
    ),
    ("Try one of these:", "Thử một trong các câu sau:"),
    (
        "Plan the next Champions Meeting for my best Pace uma and simulate it.",
        "Lập kế hoạch Champions Meeting tiếp theo cho uma Pace mạnh nhất của tôi và mô phỏng.",
    ),
    (
        "Build me a Speed-heavy deck from my MLB SSRs for a Medium Turf race.",
        "Dựng cho tôi một deck thiên Speed từ các SSR MLB của tôi cho race Medium Turf.",
    ),
    (
        "Which of my support cards give a hint for a corner recovery skill?",
        "Support card nào của tôi cho hint cho skill hồi phục ở góc cua?",
    ),
    (
        "Which of my umas should I raise to Potential 5 first, and why?",
        "Tôi nên nâng uma nào lên Potential 5 trước, và vì sao?",
    ),
    ("low", "thấp"),
    ("medium", "vừa"),
    ("high", "cao"),
    ("xhigh", "rất cao"),
    ("💭 {}", "💭 {}"),
    ("✔ Approve", "✔ Duyệt"),
    ("✖ Deny", "✖ Từ chối"),
    (
        "Settings → AI assistant can allow plan saves and result logs without asking.",
        "Cài đặt → Trợ lý AI có thể cho phép lưu kế hoạch và ghi kết quả mà không hỏi.",
    ),
    ("↶ Undo", "↶ Hoàn tác"),
    (
        "Put back the plan and Race sim as they were before this step (and every later step)",
        "Khôi phục kế hoạch và Mô phỏng đua về trạng thái trước bước này (và mọi bước sau)",
    ),
    ("{} (borrowed friend card)", "{} (card bạn mượn)"),
    ("+ friend", "+ bạn"),
    (
        "Uma 2's lead in every race, in lengths (simulation)",
        "Khoảng dẫn của Uma 2 trong từng race, tính bằng length (mô phỏng)",
    ),
    ("+{} more", "+{} nữa"),
    ("Approved: {}", "Đã duyệt: {}"),
    ("You declined.", "Bạn đã từ chối."),
    ("Answered by", "Trả lời bởi"),
    ("🆓 OpenCode · free model", "🆓 OpenCode · model miễn phí"),
    ("🔑 Claude API · your key", "🔑 Claude API · key của bạn"),
    (
        "OpenCode isn't installed yet: the download is in the chat area.",
        "OpenCode chưa được cài: phần tải xuống nằm trong khung chat.",
    ),
    (
        "⚙ Change model, key or permissions",
        "⚙ Đổi model, key hoặc quyền",
    ),
    ("Settings → AI assistant", "Cài đặt → Trợ lý AI"),
    ("What it can do", "Nó có thể làm gì"),
    (
        "📋 Draft plans, decks and skills: undo each from the chat",
        "📋 Nháp kế hoạch, deck và skill: hoàn tác từng thứ ngay trong chat",
    ),
    (
        "🏁 Run the race simulator: at most 4 runs a question unless you ask",
        "🏁 Chạy trình mô phỏng đua: tối đa 4 lần mỗi câu hỏi trừ khi bạn yêu cầu",
    ),
    (
        "💾 Save plans and log results: asks first",
        "💾 Lưu kế hoạch và ghi kết quả: hỏi trước",
    ),
    (
        "✏ Change your roster: always asks",
        "✏ Thay đổi roster: luôn hỏi",
    ),
    ("This conversation", "Cuộc trò chuyện này"),
    ("{} tokens in · {} out", "{} token vào · {} ra"),
    (
        "{} read from the prompt cache (cheaper)",
        "{} đọc từ cache prompt (rẻ hơn)",
    ),
    (
        "Free models cost nothing; the count is for interest.",
        "Model miễn phí không tốn phí; con số chỉ để tham khảo.",
    ),
    ("🗑 New conversation", "🗑 Cuộc trò chuyện mới"),
    ("Lookups ({})", "Tra cứu ({})"),
    ("Model", "Model"),
    ("Loading models…", "Đang tải model…"),
    ("🔄 Refresh models", "🔄 Làm mới model"),
    (
        "OpenCode (opencode.ai) reads your files (the roster digest, uma.guide data, plans and results) and uses UmaYu's own tools to draft plans and run the simulator. It can't edit files or run commands; plans and results it saves go through the approval cards. Free models are offered for a limited time and are weaker than Claude.",
        "OpenCode (opencode.ai) đọc file của bạn (digest roster, dữ liệu uma.guide, kế hoạch và kết quả) và dùng các tool riêng của UmaYu để nháp kế hoạch và chạy trình mô phỏng. Nó không thể sửa file hay chạy lệnh; kế hoạch và kết quả nó lưu đều đi qua thẻ duyệt. Model miễn phí được cung cấp trong thời gian giới hạn và yếu hơn Claude.",
    ),
    ("Effort", "Mức suy nghĩ"),
    (
        "Higher effort thinks longer and costs more.",
        "Mức cao hơn suy nghĩ lâu hơn và tốn phí hơn.",
    ),
    ("API key", "API key"),
    ("Remember on this PC", "Ghi nhớ trên máy này"),
    ("💾 Save key", "💾 Lưu key"),
    ("Key saved.", "Đã lưu key."),
    ("Couldn't save the key.", "Không lưu được key."),
    ("Forget saved key", "Quên key đã lưu"),
    (
        "Get a key at console.anthropic.com. Saved keys are stored in plain text under %APPDATA%\\UmaYu.",
        "Lấy key tại console.anthropic.com. Key đã lưu được giữ dạng văn bản thuần dưới %APPDATA%\\UmaYu.",
    ),
    (
        "What the assistant may do without asking",
        "Trợ lý được làm gì mà không cần hỏi",
    ),
    (
        "Drafts (the working plan, the Race sim's builds) always change at once and can be undone from the chat. Writing files asks first, unless you allow it here.",
        "Bản nháp (kế hoạch đang làm, build trong Mô phỏng đua) luôn thay đổi ngay và có thể hoàn tác từ chat. Ghi file sẽ hỏi trước, trừ khi bạn cho phép ở đây.",
    ),
    (
        "Save plans to the plans folder",
        "Lưu kế hoạch vào thư mục plans",
    ),
    (
        "Add entries to the results logs",
        "Thêm mục vào sổ ghi kết quả",
    ),
    (
        "Change your roster (limit breaks, stars, potential)",
        "Thay đổi roster (limit break, sao, potential)",
    ),
    ("Roster changes always ask.", "Thay đổi roster luôn hỏi."),
    ("key entered below", "key nhập bên dưới"),
    ("saved key", "key đã lưu"),
    ("no key set yet", "chưa đặt key"),
    ("OpenCode stopped with an error", "OpenCode dừng với lỗi"),
    ("OpenCode reported an error", "OpenCode báo lỗi"),
    ("Download failed: {}", "Tải xuống thất bại: {}"),
    (
        "The download didn't match the published checksum ({} bytes, sha256 {}), so nothing was installed.",
        "Bản tải về không khớp checksum công bố ({} byte, sha256 {}), nên không cài gì cả.",
    ),
    ("couldn't create {}: {}", "không tạo được {}: {}"),
    (
        "couldn't start curl.exe: {}",
        "không khởi chạy được curl.exe: {}",
    ),
    (
        "opencode.exe not found in the download: {}",
        "không tìm thấy opencode.exe trong bản tải: {}",
    ),
    (
        "couldn't start OpenCode: {}",
        "không khởi chạy được OpenCode: {}",
    ),
    (
        "keeps nothing, not used for training",
        "không giữ gì, không dùng để huấn luyện",
    ),
    (
        "not used for training; kept 30 days",
        "không dùng để huấn luyện; giữ 30 ngày",
    ),
    (
        "may be used to improve the model; kept 30 days",
        "có thể được dùng để cải thiện model; giữ 30 ngày",
    ),
    (
        "may be used to improve the model",
        "có thể được dùng để cải thiện model",
    ),
    (
        "NVIDIA trial: use is logged",
        "Bản dùng thử NVIDIA: hoạt động dùng được ghi log",
    ),
    ("used to train Meta models", "dùng để huấn luyện model Meta"),
    ("no data policy listed", "chưa công bố chính sách dữ liệu"),
    ("your own provider", "provider của bạn"),
    ("My account", "Tài khoản của tôi"),
    ("UmaYu folder not found.", "Không tìm thấy thư mục UmaYu."),
    (
        "Loaded {}: {} umas and {} support cards.",
        "Đã tải {}: {} uma và {} support card.",
    ),
    (
        " What was here before is saved as profiles/backups/{}.",
        " Dữ liệu trước đó đã được lưu thành profiles/backups/{}.",
    ),
    ("No data for that uma.", "Không có dữ liệu cho uma đó."),
    ("No data for that card.", "Không có dữ liệu cho card đó."),
    ("Unknown card type.", "Loại card không xác định."),
    ("Nothing to undo.", "Không có gì để hoàn tác."),
    ("Undid {} change.", "Đã hoàn tác {} thay đổi."),
    ("Undid {} changes.", "Đã hoàn tác {} thay đổi."),
    ("Welcome", "Chào mừng"),
    ("Make it yours", "Cá nhân hóa"),
    ("Your umas", "Uma của bạn"),
    ("Your cards", "Card của bạn"),
    ("All set", "Hoàn tất"),
    ("✨ Set up", "✨ Thiết lập"),
    ("Skip for now", "Bỏ qua lúc này"),
    (
        "Look around first. ⚙ Settings → Profile → Set up my collection opens this again.",
        "Cứ xem quanh đã. ⚙ Cài đặt → Hồ sơ → Thiết lập bộ sưu tập của tôi sẽ mở lại màn này.",
    ),
    ("✖ Close", "✖ Đóng"),
    ("Back to the app", "Quay lại app"),
    (
        "{} umas · {} support cards added",
        "{} uma · {} support card đã thêm",
    ),
    ("Finish →", "Hoàn tất →"),
    ("Next: your umas →", "Tiếp: uma của bạn →"),
    ("Next: your support cards →", "Tiếp: support card của bạn →"),
    (
        "Undo the last change (Ctrl+Z)",
        "Hoàn tác thay đổi cuối (Ctrl+Z)",
    ),
    ("Welcome to UmaYu", "Chào mừng đến với UmaYu"),
    (
        "Your helper for Uma Musume: Pretty Derby (Global): it knows the umas and support cards you own, plans careers for Champions Meetings and more, picks decks and skills, and races builds in a built-in simulator.",
        "Trợ thủ của bạn cho Uma Musume: Pretty Derby (Global): nó biết uma và support card bạn sở hữu, lập kế hoạch career cho Champions Meeting và hơn thế, chọn deck và skill, đồng thời cho build đua trong trình mô phỏng tích hợp.",
    ),
    (
        "First, tell it what you own. Everything stays on this computer.",
        "Trước tiên, hãy cho nó biết bạn sở hữu gì. Mọi thứ đều ở lại trên máy này.",
    ),
    ("📥 Load a profile", "📥 Tải profile"),
    (
        "Someone saved your account as a .umalord.json file, or you have a backup. Load it and you're done.",
        "Ai đó đã lưu tài khoản của bạn thành file .umalord.json, hoặc bạn có bản sao lưu. Tải lên là xong.",
    ),
    ("✨ Set up from scratch", "✨ Thiết lập từ đầu"),
    (
        "Click the umas and support cards you own: one click each. A few minutes, and you can add more any time.",
        "Bấm vào uma và support card bạn sở hữu: mỗi thứ một lần bấm. Vài phút là xong, và bạn có thể thêm bất cứ lúc nào.",
    ),
    (
        "This app already has {} umas and {} cards ({}). Setting up from scratch adds to them; ⚙ Settings → Profile → Start over empties them first.",
        "App đã có {} uma và {} card ({}). Thiết lập từ đầu sẽ thêm vào; ⚙ Cài đặt → Hồ sơ → Bắt đầu lại sẽ làm trống trước.",
    ),
    ("What should we call you?", "Chúng tôi nên gọi bạn là gì?"),
    (
        "Your trainer name: Home greets you by it, and profiles you save carry it.",
        "Tên trainer của bạn: Trang chủ dùng tên này để chào, và profile bạn lưu cũng mang tên này.",
    ),
    ("Your trainer name", "Tên trainer của bạn"),
    (
        "Leave it empty to be called Trainer.",
        "Để trống để được gọi là Trainer.",
    ),
    ("How should it look?", "Giao diện trông thế nào?"),
    (
        "Try them: the app changes as you click. All of this is in ⚙ Settings later.",
        "Cứ thử: app đổi ngay khi bạn bấm. Tất cả đều có trong ⚙ Cài đặt sau này.",
    ),
    ("Background and Seiun Sky", "Nền và Seiun Sky"),
    ("🎵 How about music?", "🎵 Thêm âm nhạc thì sao?"),
    (
        "The optional Game audio plug-in plays music directly from your local game install, 100% offline. It is off by default.",
        "Plug-in Âm thanh game tùy chọn phát nhạc trực tiếp từ bản cài đặt game trên máy, hoàn toàn ngoại tuyến 100%. Tắt theo mặc định.",
    ),
    ("Enable the Game audio plug-in", "Bật plug-in Âm thanh game"),
    (
        "💡 You can choose your game folder later in ⚙ Settings → Game audio.",
        "💡 Bạn có thể chọn thư mục game sau trong ⚙ Cài đặt → Âm thanh game.",
    ),
    (
        "This replaces what's in the app now ({}: {} umas, {} cards). That's saved to profiles/backups first, so you can load it back.",
        "Thao tác này thay thế dữ liệu hiện có trong app ({}: {} uma, {} card). Dữ liệu cũ được lưu vào profiles/backups trước, nên bạn có thể tải lại.",
    ),
    ("📥 Load this profile", "📥 Tải profile này"),
    ("Pick another", "Chọn file khác"),
    ("Load a profile", "Tải profile"),
    (
        "A profile is one .umalord.json file with someone's umas, cards, parents, plans and results. Pick one below, drag the file onto this window, or paste where it is.",
        "Profile là một file .umalord.json chứa uma, card, parent, kế hoạch và kết quả của ai đó. Chọn một file bên dưới, kéo file vào cửa sổ này, hoặc dán đường dẫn.",
    ),
    (
        "No profiles in the profiles folder or in Downloads yet.",
        "Chưa có profile nào trong thư mục profiles hoặc Downloads.",
    ),
    ("Choose", "Chọn"),
    ("{} in {}", "{} trong {}"),
    (
        "…or paste the file's full path",
        "…hoặc dán đường dẫn đầy đủ của file",
    ),
    ("📂 Open the profiles folder", "📂 Mở thư mục profiles"),
    (
        "Put a .umalord.json file here and it shows up above",
        "Đặt file .umalord.json ở đây và nó sẽ hiện bên trên",
    ),
    ("🔄 Look again", "🔄 Tìm lại"),
    ("You're all set, {}!", "Xong rồi, {}!"),
    ("Support cards", "Support card"),
    (
        "{} at 5★ · {} at 4★ · {} at 3★",
        "{} ở 5★ · {} ở 4★ · {} ở 3★",
    ),
    ("{} SSR · {} SR", "{} SSR · {} SR"),
    ("fully limit broken (MLB)", "đã full limit break (MLB)"),
    ("Maxed SSRs by type", "SSR đã max theo loại"),
    (
        "🏆 Next Champions Meeting: {}",
        "🏆 Champions Meeting tiếp theo: {}",
    ),
    ("📋 Plan for it", "📋 Lập kế hoạch cho kỳ này"),
    ("Plan a career", "Lập kế hoạch career"),
    (
        "Pick a goal and a trainee; get a deck and skills to buy.",
        "Chọn mục tiêu và trainee; nhận deck và skill cần mua.",
    ),
    ("Explore your collection", "Khám phá bộ sưu tập"),
    (
        "Everything you own, with effects at your level.",
        "Mọi thứ bạn sở hữu, kèm hiệu ứng ở cấp của bạn.",
    ),
    ("Open Home", "Mở Trang chủ"),
    (
        "Your stable at a glance and what's coming up.",
        "Đội uma của bạn trong nháy mắt và những gì sắp tới.",
    ),
    ("📝 Change what I own", "📝 Đổi những gì tôi sở hữu"),
    (
        "{} support cards and {} umas are ready to plan with.",
        "{} support card và {} uma sẵn sàng để lập kế hoạch.",
    ),
    (
        "Add your support cards any time from Collection → ＋ Add cards.",
        "Bạn có thể thêm support card bất cứ lúc nào từ Bộ sưu tập → ＋ Thêm card.",
    ),
    (
        "Your strongest type is {}: {} maxed SSR{}.",
        "Loại mạnh nhất của bạn là {}: {} SSR đã max{}.",
    ),
    (
        "Your strongest types are {} and {}: {} maxed SSRs each.",
        "Các loại mạnh nhất của bạn là {} và {}: mỗi loại {} SSR đã max.",
    ),
    ("Start over?", "Bắt đầu lại?"),
    (
        "This empties the app: {}'s {} umas, {} cards, parents, saved plans, results and notes.",
        "Thao tác này làm trống app: {1} uma, {2} card, parent, kế hoạch đã lưu, kết quả và ghi chú của {0}.",
    ),
    (
        "They're saved to profiles/backups first, so ⚙ Settings → Profile → Load a profile can bring them back.",
        "Chúng được lưu vào profiles/backups trước, nên ⚙ Cài đặt → Hồ sơ → Tải profile có thể khôi phục.",
    ),
    ("⚠ Back up and start over", "⚠ Sao lưu và bắt đầu lại"),
    (
        "Saved the old account as profiles/backups/{}.",
        "Đã lưu tài khoản cũ thành profiles/backups/{}.",
    ),
    ("Which umas do you have?", "Bạn có những uma nào?"),
    (
        "Which support cards do you have?",
        "Bạn có những support card nào?",
    ),
    (
        "Click one to add it. Hover an added one to set its stars and potential right on the tile.",
        "Bấm một thẻ để thêm. Rê chuột lên thẻ đã thêm để đặt sao và potential ngay trên thẻ.",
    ),
    (
        "Click one to add it. Hover an added one and click a diamond to set its limit break.",
        "Bấm một thẻ để thêm. Rê chuột lên thẻ đã thêm và bấm viên kim cương để đặt limit break.",
    ),
    (
        "Ctrl-click or Shift-click picks several for a bulk change. Keys: type to search (Enter adds the first match), arrows move, Space adds or removes, 1–5 sets stars (Shift for potential) or 0–4 sets LB, Delete removes, Ctrl+Z undoes.",
        "Ctrl-click hoặc Shift-click để chọn nhiều mục cho thay đổi hàng loạt. Phím: gõ để tìm (Enter thêm kết quả đầu tiên), mũi tên để di chuyển, Space thêm hoặc bỏ, 1–5 đặt sao (Shift cho potential) hoặc 0–4 đặt LB, Delete để bỏ, Ctrl+Z hoàn tác.",
    ),
    ("🔍 Search name or outfit", "🔍 Tìm tên hoặc outfit"),
    ("🔍 Search uma or card title", "🔍 Tìm uma hoặc tên card"),
    ("Only mine", "Chỉ của tôi"),
    ("Newest first", "Mới nhất trước"),
    (
        "Recent releases first: new players own the recent banners",
        "Bản phát hành gần đây trước: người chơi mới thường có banner mới",
    ),
    ("R cards", "Card R"),
    (
        "R cards are rarely worth a deck slot, so they're hidden unless you have them",
        "Card R hiếm khi đáng một ô deck, nên bị ẩn trừ khi bạn sở hữu",
    ),
    ("New picks start as", "Lựa chọn mới bắt đầu ở"),
    (
        "Set this first, then one click adds each item with these values. Set it to MLB, click every maxed card, then set it back for the rest.",
        "Đặt mục này trước, rồi mỗi lần bấm sẽ thêm món với các giá trị này. Đặt MLB, bấm mọi card đã max, rồi đặt lại cho phần còn lại.",
    ),
    ("New cards start at LB{}{}", "Card mới bắt đầu ở LB{}{}"),
    (" (maxed, MLB)", " (đã max, MLB)"),
    ("📋 Paste a list", "📋 Dán danh sách"),
    (
        "Type or paste what you own, one per line, like \"Kitasan Black MLB\"",
        "Gõ hoặc dán những gì bạn sở hữu, mỗi dòng một mục, như \"Kitasan Black MLB\"",
    ),
    ("{} picked", "Đã chọn {}"),
    ("Set limit break", "Đặt limit break"),
    (
        "Set all {} to LB{} (adds any not owned)",
        "Đặt tất cả {} sang LB{} (thêm cả mục chưa sở hữu)",
    ),
    ("✖ Remove {}", "✖ Bỏ {}"),
    ("Removed {}.", "Đã bỏ {}."),
    ("Unpick them all (Esc)", "Bỏ chọn tất cả (Esc)"),
    ("Nothing matches", "Không có kết quả"),
    (
        "Clear the search or the filters above.",
        "Xóa tìm kiếm hoặc bộ lọc phía trên.",
    ),
    ("{} of {} owned", "Sở hữu {} trên {}"),
    ("＋ Add all {} at LB{}", "＋ Thêm tất cả {} ở LB{}"),
    (
        "Adds every card in this group you haven't added, at the brush's limit break. Ctrl+Z undoes it.",
        "Thêm mọi card trong nhóm bạn chưa thêm, ở limit break của cọ. Ctrl+Z hoàn tác.",
    ),
    ("Added {} cards at LB{}.", "Đã thêm {} card ở LB{}."),
    ("Set stars and potential…", "Đặt sao và potential…"),
    (
        "Click: details on the right · stars and P on the tile set them",
        "Bấm: xem chi tiết bên phải · sao và P trên thẻ để đặt",
    ),
    ("Click to add", "Bấm để thêm"),
    (
        "{} [{}]\n{}\nRight-click for more",
        "{} [{}]\n{}\nChuột phải để xem thêm",
    ),
    ("Set limit break and level…", "Đặt limit break và level…"),
    (
        "Click: details on the right · click a diamond on the tile to set the LB",
        "Bấm: xem chi tiết bên phải · bấm viên kim cương trên thẻ để đặt LB",
    ),
    (
        "{} {} [{}]\ncard {}\n{}\nRight-click for more",
        "{} {} [{}]\ncard {}\n{}\nChuột phải để xem thêm",
    ),
    ("adds {}★ P{}", "thêm {}★ P{}"),
    ("adds LB{}", "thêm LB{}"),
    ("Edit", "Sửa"),
    (
        "Each level unlocks more of her own skills. P5 is maxed.",
        "Mỗi cấp mở thêm skill riêng của cô ấy. P5 là max.",
    ),
    (
        "The diamonds under the card in the game. All four is maxed (MLB).",
        "Các viên kim cương dưới card trong game. Đủ bốn viên là max (MLB).",
    ),
    ("Level (up to {} at LB{})", "Level (tối đa {} ở LB{})"),
    ("✖ Remove", "✖ Bỏ"),
    ("🔍 Open details", "🔍 Mở chi tiết"),
    (
        "See it in Collection (closes setup)",
        "Xem trong Bộ sưu tập (đóng màn thiết lập)",
    ),
    ("✏ {}", "✏ {}"),
    ("＋ Add", "＋ Thêm"),
    (
        "One uma or card per line, like \"Kitasan Black MLB\", \"Super Creek SSR LB3\" or \"Oguri Cap Starlight Beat 5* P5\". Typos are fine. Check the matches below, fix any that are wrong, then add them.",
        "Mỗi dòng một uma hoặc card, như \"Kitasan Black MLB\", \"Super Creek SSR LB3\" hoặc \"Oguri Cap Starlight Beat 5* P5\". Gõ sai cũng được. Kiểm tra các kết quả khớp bên dưới, sửa mục nào sai rồi thêm.",
    ),
    ("You wrote", "Bạn gõ"),
    ("Matched to", "Khớp với"),
    ("Adds as", "Thêm dưới dạng"),
    ("Sure?", "Chắc chắn?"),
    ("No match: pick one", "Không khớp: chọn một"),
    ("Best guesses", "Gợi ý tốt nhất"),
    ("Every support card", "Mọi support card"),
    ("Every uma, then every card", "Mọi uma, rồi mọi card"),
    ("no match", "không khớp"),
    ("unsure: check it", "chưa chắc: kiểm tra lại"),
    ("＋ Add {} to my collection", "＋ Thêm {} vào bộ sưu tập"),
    (
        "Missing values come from \"New picks start as\". Ctrl+Z undoes the whole list.",
        "Giá trị thiếu lấy từ \"Lựa chọn mới bắt đầu ở\". Ctrl+Z hoàn tác cả danh sách.",
    ),
    (
        "Added or updated {} from your list.",
        "Đã thêm hoặc cập nhật {} từ danh sách của bạn.",
    ),
    ("1 plan", "1 kế hoạch"),
    ("{} plans", "{} kế hoạch"),
    (
        "{} umas · {} cards · {} · saved {}",
        "{} uma · {} card · {} · lưu {}",
    ),
    ("Couldn't read {}: {}", "Không đọc được {}: {}"),
    ("Couldn't create {}: {}", "Không tạo được {}: {}"),
    ("Couldn't write {}: {}", "Không ghi được {}: {}"),
    (
        "{} isn't a umaLord profile.",
        "{} không phải profile umaLord.",
    ),
    ("{} has no roster in it.", "{} không có roster."),
    ("Couldn't remove {}: {}", "Không xóa được {}: {}"),
    // Phase 8: live data, roster warnings, the career model, meta/tier notes and auto-pick.
    ("{} days", "{} ngày"),
    ("{} h {} min", "{} giờ {} phút"),
    ("{} min", "{} phút"),
    ("in {}", "trong {}"),
    ("{} ago", "{} trước"),
    ("just now", "vừa xong"),
    ("{} min ago", "{} phút trước"),
    ("{} h ago", "{} giờ trước"),
    ("{} days ago", "{} ngày trước"),
    (", last fetched {}", ", lấy lần cuối {}"),
    (
        "{}\n{}\nChecked {}. Settings → Data and updates refreshes it.",
        "{}\n{}\nĐã kiểm tra {}. Cài đặt → Dữ liệu và cập nhật để làm mới.",
    ),
    (
        "📅 Checking the Champions Meeting schedule…",
        "📅 Đang kiểm tra lịch Champions Meeting…",
    ),
    (
        "📅 CM schedule: updated {} (umamusume.com)",
        "📅 Lịch CM: cập nhật {} (umamusume.com)",
    ),
    (
        "📅 CM schedule offline: from {} (umamusume.com)",
        "📅 Lịch CM ngoại tuyến: từ {} (umamusume.com)",
    ),
    (
        "📅 CM schedule offline: using data from {}",
        "📅 Lịch CM ngoại tuyến: dùng dữ liệu từ {}",
    ),
    (
        "📅 CM schedule: using data from {}",
        "📅 Lịch CM: dùng dữ liệu từ {}",
    ),
    ("Not fetched yet.", "Chưa lấy dữ liệu."),
    (
        "Fetching the live schedule is off (Settings → Data and updates).",
        "Đã tắt việc lấy lịch trực tuyến (Cài đặt → Dữ liệu và cập nhật).",
    ),
    ("the bundled data", "dữ liệu đi kèm"),
    (
        "Champions Meeting schedule updated from umamusume.com and umalator.app.",
        "Đã cập nhật lịch Champions Meeting từ umamusume.com và umalator.app.",
    ),
    (
        "Updated from umamusume.com; umalator.app didn't answer, so its last presets stay.",
        "Đã cập nhật từ umamusume.com; umalator.app không trả lời nên giữ các preset cũ.",
    ),
    (
        "umamusume.com didn't answer ({}); updated Umalator's estimates only.",
        "umamusume.com không trả lời ({}); chỉ cập nhật ước tính của Umalator.",
    ),
    (
        "Couldn't reach umamusume.com or umalator.app; using what was saved before.",
        "Không kết nối được umamusume.com hoặc umalator.app; dùng dữ liệu đã lưu trước đó.",
    ),
    ("unexpected reply: {}", "phản hồi không mong đợi: {}"),
    ("the site answered with code {}", "trang web trả về mã {}"),
    ("couldn't run curl.exe: {}", "không chạy được curl.exe: {}"),
    ("no answer", "không có phản hồi"),
    ("couldn't find the app bundle", "không tìm thấy app bundle"),
    (
        "no event presets in the app bundle",
        "không có preset sự kiện trong app bundle",
    ),
    ("Announced on {}", "Đã công bố trên {}"),
    ("Estimated by {}", "Ước tính bởi {}"),
    (
        "Announced on {}: the dates, course and conditions are official.",
        "Đã công bố trên {}: ngày, đường đua và điều kiện là chính thức.",
    ),
    (
        "Estimated: {} guesses the date, course and conditions until the official announcement, which can change them.",
        "Ước tính: {} đoán ngày, đường đua và điều kiện cho đến khi có thông báo chính thức, và thông báo có thể thay đổi chúng.",
    ),
    ("the bundled Umalator data", "dữ liệu Umalator đi kèm"),
    ("UmaYu folder not found", "Không tìm thấy thư mục UmaYu"),
    (
        "{} (run: python -I -X utf8 tools/update_knowledge.py)",
        "{} (chạy: python -I -X utf8 tools/update_knowledge.py)",
    ),
    (
        "Community tier list{}: Team Trials {}{}, Champions Meeting {}{}. Why: {}.{} Source: {}, sheet updated {}. Community opinion.",
        "Tier list cộng đồng{}: Team Trials {}{}, Champions Meeting {}{}. Lý do: {}.{} Nguồn: {}, sheet cập nhật {}. Ý kiến cộng đồng.",
    ),
    (", filed under {}", ", xếp vào {}"),
    (
        " Team Trials points per SP: {}.",
        " Điểm Team Trials mỗi SP: {}.",
    ),
    ("no reason given", "không nêu lý do"),
    ("meta sheet {}: {}{}", "sheet meta {}: {}{}"),
    ("; counts {}% at your LB {}", "; tính {}% ở LB {} của bạn"),
    ("{} {} deck", "deck {} {}"),
    ("{} {} borrow", "mượn {} {}"),
    (
        "{} {} deck asks for any {} card",
        "deck {} {} cần bất kỳ card {} nào",
    ),
    ("honorable mention", "gợi ý thêm"),
    ("the ○/white of {}", "bản ○/white của {}"),
    ("Races", "Các race"),
    ("Events", "Sự kiện"),
    (
        "Card row with unreadable ID: {}",
        "Dòng card có ID không đọc được: {}",
    ),
    (
        "{} {}: ID doesn't encode a rarity",
        "{} {}: ID không chứa rarity",
    ),
    (
        "{} {}: unknown type '{}'",
        "{} {}: loại không xác định '{}'",
    ),
    (
        "{} {}: unreadable LB or level",
        "{} {}: không đọc được LB hoặc level",
    ),
    (
        "{} {}: Lv{} is above the {} cap of {} at LB{}",
        "{} {}: Lv{} vượt mức tối đa {} của {} ở LB{}",
    ),
    (
        "Uma row with unreadable ID: {}",
        "Dòng uma có ID không đọc được: {}",
    ),
    (
        "{} {}: unreadable stars or potential",
        "{} {}: không đọc được sao hoặc potential",
    ),
    (
        "{} {}: stars and potential must be 1–5",
        "{} {}: sao và potential phải từ 1–5",
    ),
    (
        "That ID doesn't encode a rarity.",
        "ID đó không chứa rarity.",
    ),
    (
        "Level must be 1–{} for {} at LB{}.",
        "Level phải từ 1–{} cho {} ở LB{}.",
    ),
    (
        "Couldn't find the card table's header.",
        "Không tìm thấy header của bảng card.",
    ),
    (
        "Couldn't find the card table under the rarity heading.",
        "Không tìm thấy bảng card dưới tiêu đề rarity.",
    ),
    (
        "Stars and potential run 1–5.",
        "Sao và potential chạy từ 1–5.",
    ),
    (
        "Couldn't find the uma table in account/umas.md.",
        "Không tìm thấy bảng uma trong account/umas.md.",
    ),
    ("Tie-break", "Phân định hòa"),
    ("Strong", "Mạnh"),
    (
        "Simulation only: the meta sheet's skill advice doesn't count.",
        "Chỉ mô phỏng: gợi ý skill của sheet meta không được tính.",
    ),
    (
        "The meta sheet's advice only decides between skills the simulation finds about equal (+0.05 L per unit of backing).",
        "Gợi ý của sheet meta chỉ phân định giữa các skill mà mô phỏng thấy gần bằng nhau (+0.05 L mỗi đơn vị ủng hộ).",
    ),
    (
        "The meta sheet's skills first, as long as they can trigger on the course (+2 L per unit of backing).",
        "Ưu tiên skill của sheet meta, miễn là chúng kích hoạt được trên đường đua (+2 L mỗi đơn vị ủng hộ).",
    ),
    ("Win the CM race (lengths)", "Thắng race CM (theo length)"),
    ("Team Trials (consistency)", "Team Trials (độ ổn định)"),
    ("Rating / parent (points)", "Rating / parent (điểm)"),
    (
        "The set that gains the most lengths on average in the plan's race.",
        "Bộ skill tăng nhiều length nhất tính trung bình trong race của kế hoạch.",
    ),
    (
        "Each skill's gain is weighted by how often it triggers, so cheap, reliable skills win.",
        "Mức tăng của mỗi skill được nhân theo tần suất kích hoạt, nên skill rẻ và ổn định sẽ thắng.",
    ),
    (
        "The most skill evaluation points within the SP budget, for the uma's rank: an SS rank parent makes better sparks (uma.guide, Parenting). Points come from uma.guide's skill data. The game also scales skills tied to a distance or style by the uma's aptitude, which isn't modelled. No racing.",
        "Bộ skill có nhiều điểm đánh giá nhất trong ngân sách SP, theo rank của uma: parent hạng SS tạo spark tốt hơn (uma.guide, Parenting). Điểm lấy từ dữ liệu skill của uma.guide. Game cũng nhân skill gắn với cự ly hoặc style theo aptitude của uma, điều này chưa được mô hình hóa. Không chạy race.",
    ),
    (
        "Screening {} skills in realistic races…",
        "Đang sàng lọc {} skill trong các race thực tế…",
    ),
    (
        "Screened {} skills alone in realistic races (wit checks and rushing on); kept {} by gain and by gain per SP, plus every pinned and meta-backed one.",
        "Đã sàng lọc {} skill riêng lẻ trong các race thực tế (bật wit check và rushing); giữ {} theo mức tăng và mức tăng mỗi SP, cộng mọi skill đã ghim và được sheet meta ủng hộ.",
    ),
    ("📌 {} (pinned)", "📌 {} (đã ghim)"),
    (
        "Stopped: nothing else fits the {} SP left.",
        "Dừng: không còn skill nào vừa với {} SP còn lại.",
    ),
    (
        "Choosing skill {}: racing {} options on top of {} chosen…",
        "Chọn skill {}: đang đua {} lựa chọn trên {} skill đã chọn…",
    ),
    (
        "Stopped: the best next skill, {}, adds only {}.",
        "Dừng: skill tốt nhất tiếp theo, {}, chỉ thêm {}.",
    ),
    (", upgrading {}", ", nâng cấp {}"),
    (" (meta +{})", " (meta +{})"),
    (
        "{}. {}: {} on top of the set{}, for {} SP{},{} — {} SP left.",
        "{}. {}: {} cộng thêm vào bộ{}, với {} SP{},{} — còn {} SP.",
    ),
    (", best that fits", ", tốt nhất còn vừa"),
    (
        "Trying {} swaps (round {})…",
        "Đang thử {} hoán đổi (vòng {})…",
    ),
    ("Swap: {} out, {} in: {}.", "Hoán đổi: bỏ {}, thêm {}: {}."),
    (
        "No swap of one chosen skill for another does better.",
        "Không có hoán đổi skill nào cho kết quả tốt hơn.",
    ),
    (
        "Checking the set in {} races…",
        "Đang kiểm tra bộ skill trong {} race…",
    ),
    (
        "Checked in {} races: {} L (95%: {} to {}) against her unique alone. Their single gains add up to {} L: skills overlap and share the same stretches of the race.",
        "Đã kiểm tra trong {} race: {} L (95%: {} đến {}) so với riêng unique của cô ấy. Tổng mức tăng riêng của chúng là {} L: các skill chồng lấn và chia sẻ cùng những đoạn đường đua.",
    ),
    (
        "No umalord-plan block in that file.",
        "Tệp đó không có block umalord-plan.",
    ),
    ("No simulator data.", "Chưa có dữ liệu mô phỏng."),
    ("Unknown course.", "Không rõ đường đua."),
    // Game Audio (Phase 1 & 2)
    ("Game audio", "Âm thanh game"),
    (
        "Connect to local Uma Musume game installation files to enable Audio Player, Jukebox, and Live Music.",
        "Kết nối với tệp cài đặt game Uma Musume cục bộ để bật Audio Player, Jukebox và Live Music.",
    ),
    ("Enable Game Audio plug-in", "Bật plug-in Âm thanh game"),
    (
        "Enable Game Audio features using local game assets.",
        "Bật tính năng Âm thanh game sử dụng dữ liệu game cục bộ.",
    ),
    (
        "Game audio is currently disabled. Enable it above to configure and load game tracks.",
        "Tính năng âm thanh game hiện đang tắt. Bật ở trên để cấu hình và tải các bài nhạc trong game.",
    ),
    ("Game directory", "Thư mục game"),
    (
        "Path to the Persistent directory containing dat, meta, and master folders.",
        "Đường dẫn đến thư mục Persistent chứa các thư mục dat, meta và master.",
    ),
    ("e.g. C:\\...\\Persistent", "vd: C:\\...\\Persistent"),
    ("🔍 Auto-detect", "🔍 Dò tự động"),
    (
        "Scan for Global, Steam JP, and DMM JP installations",
        "Quét tìm các bản cài đặt Global, Steam JP và DMM JP",
    ),
    ("🔄 Reload", "🔄 Tải lại"),
    (
        "Reload manifest and master databases from disk",
        "Tải lại manifest và cơ sở dữ liệu master từ đĩa",
    ),
    ("Detected games", "Bản cài phát hiện"),
    (
        "Found game installations on this computer.",
        "Các bản cài đặt game tìm thấy trên máy tính này.",
    ),
    ("Valid", "Hợp lệ"),
    ("Incomplete", "Chưa đủ tệp"),
    (
        "Current status of the game data reader.",
        "Trạng thái hiện tại của trình đọc dữ liệu game.",
    ),
    ("No game directory selected.", "Chưa chọn thư mục game."),
    ("Idle. Ready to load.", "Sẵn sàng tải."),
    ("Scanning for installations…", "Đang quét tìm bản cài đặt…"),
    (
        "Loading manifest and master databases…",
        "Đang tải manifest và cơ sở dữ liệu master…",
    ),
    ("Loaded successfully.", "Đã tải thành công."),
    (
        "Manifest: {} audio entries · Master: {} live songs, {} jukebox tracks",
        "Manifest: {} mục âm thanh · Master: {} bài live, {} bài jukebox",
    ),
    ("Advanced: Key", "Nâng cao: Khóa giải mã"),
    (
        "Custom hex encryption key for meta database (optional).",
        "Khóa giải mã hex tùy chỉnh cho database meta (tùy chọn).",
    ),
    (
        "Leave blank to use standard game keys",
        "Để trống để dùng khóa chuẩn của game",
    ),
    ("▶ Play", "▶ Phát"),
    ("⏸ Pause", "⏸ Tạm dừng"),
    ("⏹ Stop", "⏹ Dừng"),
    ("Volume:", "Âm lượng:"),
    // Game Audio (Phase 3: Tab Music & Files sub-tab)
    ("🎵 Music", "🎵 Âm thanh game"),
    ("Music", "Âm thanh game"),
    (
        "Game audio library: browse sound files, jukebox songs, and live music.",
        "Thư viện âm thanh game: duyệt tệp âm thanh, bài hát jukebox và nhạc sân khấu live.",
    ),
    ("📁 Files", "📁 Tệp âm thanh"),
    ("📁 Files  {}", "📁 Tệp âm thanh  {}"),
    (
        "Browse and play game audio files (ACB/AWB)",
        "Duyệt và phát các tệp âm thanh của game (ACB/AWB)",
    ),
    ("📻 Jukebox", "📻 Hộp nhạc"),
    ("📻 Jukebox  {}", "📻 Hộp nhạc  {}"),
    (
        "Game jukebox songs (Phase 4)",
        "Bài hát trong Hộp nhạc (Phase 4)",
    ),
    ("🎤 Live", "🎤 Sân khấu Live"),
    ("🎤 Live  {}", "🎤 Sân khấu Live  {}"),
    (
        "Multi-channel live stage player (Phase 5)",
        "Trình phát nhạc sân khấu Live đa kênh (Phase 5)",
    ),
    (
        "Browse CRIWARE audio files from game storage (ACB cue sheets and AWB containers). Select a file to inspect cues, play, or export WAV.",
        "Duyệt các tệp âm thanh CRIWARE từ bộ lưu trữ game (ACB cue sheet và AWB container). Chọn một tệp để xem các track, phát hoặc xuất ra WAV.",
    ),
    (
        "Jukebox music collection: short and game-size versions with cuesheet linkage (scheduled for Phase 4).",
        "Bộ sưu tập bài hát Hộp nhạc: phiên bản ngắn và bản kích thước game liên kết qua cue sheet (dự kiến trong Phase 4).",
    ),
    (
        "Live performance stage player: multi-track mixer with karaoke lyrics and member line-up (scheduled for Phase 5).",
        "Trình phát biểu diễn sân khấu Live: bộ trộn đa track kèm lời karaoke và đội hình nhân vật (dự kiến trong Phase 5).",
    ),
    ("⏳ Exporting WAV...", "⏳ Đang xuất WAV..."),
    ("Filters", "Bộ lọc"),
    ("🔍 Search file name (Ctrl+F)", "🔍 Tìm tên tệp (Ctrl+F)"),
    ("CATEGORY", "PHÂN LOẠI"),
    (
        "Filter by game audio folder category",
        "Lọc theo thư mục phân loại âm thanh game",
    ),
    ("📋 All", "📋 Tất cả"),
    ("All sound files", "Tất cả tệp âm thanh"),
    ("🎵 BGM", "🎵 Nhạc nền"),
    ("Background music", "Nhạc nền"),
    ("🎬 Cutin", "🎬 Hoạt cảnh"),
    ("Cut-in scene sounds", "Âm thanh cắt cảnh"),
    ("🔔 Jingle", "🔔 Nhạc ngắn"),
    ("Short jingles and fanfares", "Nhạc ngắn và khúc dạo đầu"),
    ("Live music archives", "Kho nhạc sân khấu live"),
    ("⚙ System", "⚙ Hệ thống"),
    ("System sound effects", "Hiệu ứng âm thanh hệ thống"),
    ("🗣 Voice", "🗣 Giọng nói"),
    ("Character voice lines", "Giọng nói nhân vật"),
    ("STORAGE", "LƯU TRỮ"),
    (
        "Filter by local file download status",
        "Lọc theo trạng thái tải tệp trên máy",
    ),
    ("Downloaded files only", "Chỉ tệp đã tải về"),
    (
        "Only show audio files that are downloaded and present on disk",
        "Chỉ hiển thị các tệp âm thanh đã tải về và có sẵn trên ổ đĩa",
    ),
    ("Showing {} of {} files", "Đang hiện {} / {} tệp"),
    ("✕ Reset filters", "✕ Đặt lại bộ lọc"),
    ("Filtering audio files...", "Đang lọc danh sách tệp..."),
    ("No sound files found", "Không tìm thấy tệp âm thanh nào"),
    (
        "Try adjusting your search terms or clearing category filters.",
        "Thử điều chỉnh từ khóa tìm kiếm hoặc xóa bộ lọc phân loại.",
    ),
    ("Kind", "Loại"),
    ("Asset Name", "Tên tệp asset"),
    ("Size", "Kích thước"),
    ("Status", "Trạng thái"),
    ("✓ Ready", "✓ Sẵn sàng"),
    ("⏳ Missing", "⏳ Chưa tải"),
    (
        "Not downloaded in game. Download game data to play.",
        "Chưa tải trong game. Hãy tải thêm dữ liệu game để phát.",
    ),
    ("Track Details", "Chi tiết track"),
    ("Asset:", "Tệp asset:"),
    ("Length:", "Dung lượng:"),
    ("Storage hash:", "Hash lưu trữ:"),
    (
        "⚠️ File not downloaded in game",
        "⚠️ Tệp chưa được tải về trong game",
    ),
    (
        "This audio file is registered in the game's manifest database but its data file is not present on your PC. Please download additional assets within the game to enable playback.",
        "Tệp âm thanh này có trong manifest của game nhưng dữ liệu chưa được tải về máy của bạn. Hãy tải thêm tài nguyên trong game để có thể phát.",
    ),
    ("▶ Play cue #0", "▶ Phát cue #0"),
    ("💾 Export all WAV", "💾 Xuất toàn bộ WAV"),
    (
        "Export all waves in this file to exports/audio/",
        "Xuất tất cả các track trong tệp này ra thư mục exports/audio/",
    ),
    (
        "Queued all tracks for WAV export.",
        "Đã đưa tất cả track vào hàng đợi xuất WAV.",
    ),
    ("Wave Cues", "Danh sách Wave Cue"),
    ("Reading audio cues...", "Đang đọc các wave cue..."),
    ("Cannot read cues", "Không thể đọc cue"),
    (
        "No audio waves inside this container.",
        "Không có wave âm thanh nào trong container này.",
    ),
    (
        "Export this track as 16-bit PCM WAV",
        "Xuất track này thành file WAV PCM 16-bit",
    ),
    ("Play this cue", "Phát cue này"),
    ("Exporting to {}", "Đang xuất sang {}"),
    ("📻 Jukebox Music Player", "📻 Trình phát nhạc Hộp nhạc"),
    (
        "Browse and play in-game Jukebox music tracks in both Short and Game-size versions.",
        "Duyệt và phát các bài nhạc Hộp nhạc trong game ở cả phiên bản ngắn và bản game-size.",
    ),
    (
        "Loaded {} Jukebox tracks from master.mdb",
        "Đã nạp {} bài hát Jukebox từ master.mdb",
    ),
    (
        "Full player controls will be implemented in Phase 4.",
        "Trình phát đầy đủ sẽ được triển khai trong Phase 4.",
    ),
    (
        "🎤 Live Music Stage Player",
        "🎤 Trình phát nhạc Sân khấu Live",
    ),
    (
        "Multi-track playback mixer with karaoke lyrics synchronization and custom character vocal lineups.",
        "Bộ trộn phát nhạc đa track đồng bộ lời karaoke và tùy chỉnh đội hình giọng ca nhân vật.",
    ),
    (
        "Loaded {} Live performances from master.mdb",
        "Đã nạp {} bài biểu diễn Live từ master.mdb",
    ),
    (
        "Full score parsing and multi-track mixing will be implemented in Phase 5.",
        "Đọc sheet nhạc và trộn đa track đầy đủ sẽ được triển khai trong Phase 5.",
    ),
    ("Pause", "Tạm dừng"),
    ("Play", "Phát"),
    ("Stop", "Dừng"),
    ("Loop", "Lặp lại"),
    (
        "Seamless track repeat",
        "Lặp lại bài hát mượt mà không ngắt quãng",
    ),
    ("Close player", "Đóng trình phát"),
    // Phase 4 — Jukebox
    (
        "Game jukebox songs collection",
        "Bộ sưu tập bài hát Hộp nhạc",
    ),
    (
        "Jukebox music collection: short and game-size versions with cuesheet linkage and instant playback.",
        "Bộ sưu tập bài hát Hộp nhạc: phiên bản ngắn và bản kích thước game liên kết qua cue sheet và phát tức thì.",
    ),
    (
        "🔍 Search title or cue (Ctrl+F)",
        "🔍 Tìm tên bài hoặc cue sheet (Ctrl+F)",
    ),
    ("DEFAULT VERSION", "PHIÊN BẢN MẶC ĐỊNH"),
    (
        "Preferred playback version for quick-play",
        "Phiên bản ưu tiên khi bấm phát nhanh",
    ),
    ("⚡ Short", "⚡ Bản ngắn (Short)"),
    (
        "Shortened preview version (~20-35 seconds)",
        "Phiên bản rút gọn (~20-35 giây)",
    ),
    ("🎼 Game-size", "🎼 Bản đầy đủ (Game-size)"),
    (
        "Full in-game length track version",
        "Phiên bản đầy đủ độ dài trong game",
    ),
    ("AVAILABILITY", "TÌNH TRẠNG TẢI"),
    (
        "Filter by storage and visibility",
        "Lọc theo trạng thái tải về và hiển thị",
    ),
    ("Hide hidden tracks", "Ẩn bài bị ẩn"),
    (
        "Hide unreleased or secret jukebox tracks flagged as hidden",
        "Ẩn các bài hát chưa mở khóa hoặc bí mật được gắn cờ ẩn trong game",
    ),
    ("STATISTICS", "THỐNG KÊ"),
    (
        "Loaded jukebox songs overview",
        "Tổng quan danh sách bài hát Hộp nhạc",
    ),
    ("Total songs: {}", "Tổng số bài hát: {}"),
    ("Ready on disk: {}", "Sẵn sàng trên máy: {}"),
    ("↺ Reset filters", "↺ Đặt lại bộ lọc"),
    (
        "Reset all search queries and filters to default",
        "Đặt lại toàn bộ từ khóa tìm kiếm và bộ lọc về mặc định",
    ),
    ("💡 Jukebox Tip", "💡 Mẹo Hộp nhạc"),
    (
        "If a Short version is not downloaded in the game yet, UmaYu will automatically fallback to the Game-size version, and vice-versa.",
        "Nếu bản ngắn chưa được tải trong game, UmaYu sẽ tự động phát bản đầy đủ (Game-size) thay thế, và ngược lại.",
    ),
    (
        "No jukebox songs found",
        "Không tìm thấy bài hát nào trong Hộp nhạc",
    ),
    (
        "Try adjusting your search terms or unchecking 'Downloaded only' filter.",
        "Thử điều chỉnh từ khóa tìm kiếm hoặc bỏ chọn 'Chỉ tệp đã tải về'.",
    ),
    ("Downloaded only", "Chỉ bài hát đã tải về"),
    ("Showing {} songs", "Đang hiển thị {} bài hát"),
    ("Song Title", "Tên bài hát"),
    ("Length", "Thời lượng"),
    ("Short", "Bản Short"),
    ("Game-size", "Bản Game"),
    (
        "Play Short version (auto-fallback to Game-size)",
        "Phát bản ngắn (tự động chuyển sang bản Game-size nếu thiếu)",
    ),
    (
        "Play Game-size version (auto-fallback to Short)",
        "Phát bản Game-size (tự động chuyển sang bản ngắn nếu thiếu)",
    ),
    ("Deselect song", "Bỏ chọn bài hát"),
    (
        "Music ID: #{} • Sort Order: {} • Hidden: {}",
        "Music ID: #{} • Thứ tự sắp xếp: {} • Bài ẩn: {}",
    ),
    ("⚡ Short Version", "⚡ Phiên bản ngắn (Short)"),
    ("🎼 Game-size Version", "🎼 Phiên bản đầy đủ (Game-size)"),
    ("✓ Ready on disk", "✓ Sẵn sàng trên máy"),
    ("⏳ Not downloaded in game", "⏳ Chưa tải trong game"),
    ("Cue sheet: {}", "Cue sheet: {}"),
    ("Length: {}", "Thời lượng: {}"),
    ("▶ Play Short", "▶ Phát bản Short"),
    ("💾 Export WAV", "💾 Xuất WAV"),
    (
        "Play the Short version (wave 0)",
        "Phát phiên bản ngắn (wave 0)",
    ),
    (
        "Short version not downloaded — will auto-fallback to Game-size version",
        "Bản ngắn chưa tải — sẽ tự động chuyển sang phát bản Game-size",
    ),
    (
        "Export Short version to WAV (exports/audio/jukebox/)",
        "Xuất bản ngắn sang tệp WAV (exports/audio/jukebox/)",
    ),
    ("▶ Play Game-size", "▶ Phát bản Game-size"),
    (
        "Play the Game-size version (wave 0)",
        "Phát phiên bản đầy đủ Game-size (wave 0)",
    ),
    (
        "Game-size version not downloaded — will auto-fallback to Short version",
        "Bản Game-size chưa tải — sẽ tự động chuyển sang phát bản ngắn",
    ),
    (
        "Export Game-size version to WAV (exports/audio/jukebox/)",
        "Xuất bản Game-size sang tệp WAV (exports/audio/jukebox/)",
    ),
    ("Cover", "Ảnh bìa"),
    (
        "Short version not downloaded. Playing Game-size version instead.",
        "Bản ngắn chưa được tải về. Tự động chuyển sang phát bản Game-size thay thế.",
    ),
    (
        "Game-size version not downloaded. Playing Short version instead.",
        "Bản Game-size chưa được tải về. Tự động chuyển sang phát bản ngắn thay thế.",
    ),
    (
        "Cannot export: file is not downloaded in game yet.",
        "Không thể xuất: tệp âm thanh chưa được tải về trong game.",
    ),
    (
        "Cuesheet not found in manifest.",
        "Không tìm thấy cue sheet trong manifest.",
    ),
    // Phase 5 — Live Music Player
    (
        "Multi-track live stage player",
        "Trình phát sân khấu Live đa kênh",
    ),
    (
        "Multi-track live stage player with synchronized karaoke lyrics and customizable character lineup.",
        "Trình phát sân khấu Live đa kênh với lời karaoke đồng bộ và tùy biến đội hình nhân vật.",
    ),
    (
        "Live performance stage player: multi-track mixer with karaoke lyrics and member line-up.",
        "Trình phát sân khấu Live: bộ trộn đa track kèm lời bài hát karaoke và tùy biến đội hình ca sĩ.",
    ),
    (
        "Search live song or ID (Ctrl+F)",
        "Tìm tên bài hát hoặc ID (Ctrl+F)",
    ),
    (
        "Loaded live songs overview",
        "Tổng quan danh sách bài hát Live",
    ),
    ("Total performances: {}", "Tổng số bài Live: {}"),
    ("Performers", "Đội hình"),
    ("Stage size: {} performers", "Sân khấu: {} ca sĩ"),
    ("Stage: {} performers", "Sân khấu: {} người"),
    ("Play Live performance", "Phát bài biểu diễn Live"),
    ("Deselect live performance", "Bỏ chọn bài biểu diễn Live"),
    ("AUDIO EFFECTS & SFX", "HIỆU ỨNG ÂM THANH & SFX"),
    (
        "Cheering & audience SFX vs clean instrumental",
        "Hiệu ứng tiếng reo hò cổ vũ so với bản nhạc nền sạch",
    ),
    ("🔊 SFX 01 (Cheer)", "🔊 SFX 01 (Cổ vũ)"),
    ("🔊 SFX 01 (Cổ vũ)", "🔊 SFX 01 (Cổ vũ)"),
    (
        "Cheering & sound effects track",
        "Bản nhạc nền có kèm tiếng reo hò và hiệu ứng âm thanh",
    ),
    ("🎵 SFX 02 (Clean)", "🎵 SFX 02 (Bản sạch)"),
    ("🎵 SFX 02 (Bản sạch)", "🎵 SFX 02 (Bản sạch)"),
    (
        "Clean instrumental track without audience cheer",
        "Bản nhạc nền sạch không kèm tiếng reo hò của khán giả",
    ),
    ("MASTER MUTE CONTROLS", "ĐIỀU KHIỂN TẮT TIẾNG TỔNG"),
    (
        "Independent mute controls for BGM and vocal channels",
        "Điều khiển tắt tiếng độc lập cho kênh nhạc nền và kênh giọng hát",
    ),
    (
        "🔇 Mute Oke (Vocals only)",
        "🔇 Tắt nhạc nền (Chỉ nghe giọng)",
    ),
    (
        "Mute background instrumental track to isolate character voices",
        "Tắt tiếng nhạc nền để chỉ nghe riêng giọng hát của các nhân vật",
    ),
    (
        "🔇 Mute Vocals (Instrumental only)",
        "🔇 Tắt giọng hát (Chỉ nghe nhạc)",
    ),
    (
        "Mute all vocal channels to listen to the karaoke instrumental",
        "Tắt tất cả các kênh giọng hát để thưởng thức nhạc nền hòa tấu karaoke",
    ),
    ("Lineup & Setup", "Đội hình & Cài đặt"),
    (
        "Select and configure lineup",
        "Chọn bài và cài đặt đội hình",
    ),
    ("▶ Play Live", "▶ Phát Live"),
    ("Resume Live", "Tiếp tục Live"),
    ("Loading...", "Đang tải..."),
    ("Pause live playback", "Tạm dừng phát Live"),
    (
        "Resume paused live playback",
        "Tiếp tục phát Live đang tạm dừng",
    ),
    (
        "Start multi-track live performance",
        "Bắt đầu phát bài biểu diễn Live đa kênh",
    ),
    ("Decoding live tracks...", "Đang giải mã âm thanh Live..."),
    (
        "Start or resume multi-track live performance",
        "Bắt đầu hoặc tiếp tục phát bài biểu diễn Live đa kênh",
    ),
    ("Export Live Mix WAV", "Xuất Live Mix WAV"),
    (
        "Export full multi-track live mix to exports/audio/live/",
        "Xuất toàn bộ bản hòa âm đa kênh Live ra thư mục exports/audio/live/",
    ),
    ("🎲 Randomize Lineup", "🎲 Ngẫu nhiên đội hình"),
    (
        "Randomly shuffle permitted characters for this performance",
        "Xáo trộn ngẫu nhiên các nhân vật được phép biểu diễn cho bài hát này",
    ),
    ("🔄 Reset Lineup", "🔄 Đặt lại đội hình"),
    (
        "Reset lineup to default stage configuration",
        "Khôi phục đội hình về cấu hình sân khấu ban đầu",
    ),
    (
        "🎤 Synchronized Karaoke Lyrics",
        "🎤 Lời bài hát Karaoke đồng bộ",
    ),
    (
        "Play a Live performance to see synchronized lyrics.",
        "Phát một bản Live để xem lời hát đồng bộ.",
    ),
    (
        "Play this Live performance to see synchronized lyrics.",
        "Phát bài Live này để xem lời hát đồng bộ.",
    ),
    (
        "Could not read lyrics data",
        "Không đọc được dữ liệu lời hát",
    ),
    (
        "This song has no lyrics data in the game files.",
        "Bài này không có dữ liệu lời hát trong tệp game.",
    ),
    (
        "Lyrics data is not downloaded in game yet.",
        "Dữ liệu lời hát chưa được tải về trong game.",
    ),
    (
        "This song has no lyrics (instrumental).",
        "Bài này không có lời hát (thuần hòa tấu).",
    ),
    ("Line {}/{}", "Dòng {}/{}"),
    ("STAGE PERFORMER LINEUP", "ĐỘI HÌNH BIỂU DIỄN TRÊN SÂN KHẤU"),
    (
        "Configure character assignment, singing mode, solo, and volume for each stage slot",
        "Cấu hình nhân vật, chế độ hát, solo và âm lượng cho từng vị trí trên sân khấu",
    ),
    ("Slot {} (Center)", "Vị trí {} (Trung tâm)"),
    ("Slot {}", "Vị trí {}"),
    ("⭐ Solo", "⭐ Solo"),
    (
        "Solo this performer (mutes other non-solo singers)",
        "Hát đơn ca sĩ này (tạm tắt các ca sĩ khác không bật solo)",
    ),
    ("Auto (Part)", "Tự động (Nhịp)"),
    (
        "Follows stage part triggers dynamically",
        "Tự động hát hoặc nghỉ theo đúng nhịp trigger của bài hát",
    ),
    ("Main", "Hát chính"),
    ("Main (Chính)", "Hát chính"),
    (
        "Force sing main vocal track (wave 0)",
        "Bắt buộc hát track giọng chính (wave 0) xuyên suốt",
    ),
    ("Extra", "Hát bè/phụ"),
    ("Extra (Phụ)", "Hát bè/phụ"),
    (
        "Force sing extra vocal track (wave 1)",
        "Bắt buộc hát track bè/hòa âm phụ (wave 1) xuyên suốt",
    ),
    ("Merge", "Cả hai giọng"),
    ("Merge (Cả hai)", "Cả hai giọng"),
    (
        "Merge and sing both main and extra vocals",
        "Hòa trộn và phát cả giọng chính lẫn giọng bè",
    ),
    ("Mute", "Tắt tiếng"),
    ("Mute (Tắt)", "Tắt tiếng"),
    (
        "Mute this performer completely",
        "Tắt hoàn toàn tiếng ca sĩ ở vị trí này",
    ),
    ("Ready on disk", "Sẵn sàng trên máy"),
    (
        "Missing vocal file in game",
        "Chưa tải tệp giọng hát trong game",
    ),
    (
        "No live performances found",
        "Không tìm thấy bài biểu diễn Live nào",
    ),
    ("Singing mode: {}", "Chế độ hát: {}"),
    ("Active singers: {}", "Đang hát: {}"),
    (
        "Live oke track missing on disk. Please download it in game.",
        "Tệp nhạc nền Live chưa có trên máy. Hãy tải về trong game.",
    ),
    ("Play cue #0", "Phát cue #0"),
    (
        "Only show songs that have at least one audio file downloaded on disk",
        "Chỉ hiển thị các bài hát có ít nhất một tệp âm thanh đã tải về trên máy",
    ),
    ("Yes", "Có"),
    ("No", "Không"),
    ("Asset: {}", "Tệp asset: {}"),
    ("SEARCH LIVE", "TÌM KIẾM LIVE"),
    ("Showing {} performances", "Đang hiển thị {} bài biểu diễn"),
    ("AUDIO FX", "HIỆU ỨNG ÂM THANH"),
    ("Play Live", "Phát Live"),
    ("💾 Export Live WAV", "💾 Xuất live WAV"),
    ("Show lyrics", "Hiện phụ đề"),
    (
        "Toggle synchronized karaoke lyrics display",
        "Bật/tắt hiển thị lời bài hát karaoke đồng bộ",
    ),
    ("🎤 Main", "🎤 Hát chính"),
    ("🎤 [Main Vocal]", "🎤 [Hát chính]"),
    (
        "This performer is forced to sing the main vocal",
        "Ca sĩ này đang được ép hát giọng chính",
    ),
    (
        "Currently forced as main vocal. Click to revert to Auto (Part).",
        "Đang ép hát giọng chính. Nhấn để quay lại Tự động (Nhịp).",
    ),
    (
        "Set this Uma as the only main vocal; other slots revert to Auto (Part).",
        "Đặt Uma này làm giọng chính duy nhất; các vị trí khác quay về Tự động (Nhịp).",
    ),
    ("🎤 Main vocal:", "🎤 Hát chính:"),
    (
        "Main vocal voice file is not downloaded in game yet.",
        "Tệp giọng hát chính chưa tải về trong game.",
    ),
    ("No vocal data", "Không có dữ liệu giọng hát"),
    ("Chara #{}", "Nhân vật #{}"),
    ("Main vocal", "Giọng chính"),
    (
        "Toggle the main vocal wave (Wave 0) for this performer. When off, only extra/harmony voice plays if active.",
        "Bật/tắt riêng wave giọng chính (Wave 0) của ca sĩ này. Khi tắt, chỉ phát giọng phụ/hòa âm nếu có.",
    ),
    ("🔇 [Main Off]", "🔇 [Tắt giọng chính]"),
    (
        "Main vocal wave is muted for this performer",
        "Wave giọng chính của ca sĩ này đã bị tắt",
    ),
    // Phase 5 — WAV Export Directory Customization & Quick Explorer Access
    (
        "Export all waves in this file to exports/audio/files/",
        "Xuất tất cả các track trong tệp này ra thư mục exports/audio/files/",
    ),
    (
        "Queued all tracks for WAV export to {}.",
        "Đã xếp hàng xuất tất cả track WAV vào {}.",
    ),
    ("📂 Export folder", "📂 Thư mục xuất"),
    (
        "Open the WAV audio export directory in Windows Explorer",
        "Mở thư mục xuất âm thanh WAV trong Windows Explorer",
    ),
    ("Export directory", "Thư mục xuất"),
    (
        "Custom base folder where exported WAV files are saved (files, jukebox, live).",
        "Thư mục gốc tùy chỉnh lưu trữ các tệp WAV xuất ra (files, jukebox, live).",
    ),
    (
        "Reset export directory to default (exports/audio)",
        "Đặt lại thư mục xuất về mặc định (exports/audio)",
    ),
    (
        "Open export folder in Windows Explorer",
        "Mở thư mục xuất trong Windows Explorer",
    ),
    ("📂 Open folder", "📂 Mở thư mục"),
    ("🔄 Reset", "🔄 Đặt lại"),
    // Phase 6 — Dual Global & JP Data Catalogs & Rapid Region Switching
    (
        "Switch to Global game data",
        "Chuyển sang dữ liệu game bản Global",
    ),
    ("Switch to JP game data", "Chuyển sang dữ liệu game bản JP"),
    (
        "game directory is not configured.",
        "thư mục game chưa được cấu hình.",
    ),
    (
        "Please configure the game Persistent directory in Settings → Game audio.",
        "Vui lòng cấu hình thư mục Persistent của game trong Cài đặt → Âm thanh game.",
    ),
    (
        "Please configure it in Settings → Game audio.",
        "Vui lòng cấu hình thư mục trong Cài đặt → Âm thanh game.",
    ),
    ("Loading game data", "Đang tải dữ liệu game"),
    ("⚙ Open Settings", "⚙ Mở Cài đặt"),
    ("↻ Rescan", "↻ Quét lại"),
    ("↻ Scanning disk...", "↻ Đang quét ổ đĩa..."),
    (
        "Rescan game files on disk to update downloaded status",
        "Quét lại các tệp game trên ổ đĩa để cập nhật trạng thái đã tải về",
    ),
    ("Active region", "Phiên bản hoạt động"),
    (
        "Select active game data region (Global or JP) used in Music.",
        "Chọn phiên bản dữ liệu game hoạt động (Global hoặc JP) dùng trong Âm thanh game.",
    ),
    ("Use Global game data", "Sử dụng dữ liệu game bản Global"),
    ("Use JP game data", "Sử dụng dữ liệu game bản JP"),
    ("Global version", "Phiên bản Global"),
    ("Global directory", "Thư mục Global"),
    (
        "Path to the Persistent directory containing dat, meta, and master folders for Global.",
        "Đường dẫn đến thư mục Persistent chứa dat, meta và master của bản Global.",
    ),
    (
        "e.g. C:\\...\\LocalLow\\Cygames\\umamusume",
        "vd: C:\\...\\LocalLow\\Cygames\\umamusume",
    ),
    ("📁 Browse…", "📁 Duyệt…"),
    (
        "Select Global game directory via folder picker",
        "Chọn thư mục game Global bằng cửa sổ chọn thư mục",
    ),
    ("Select Global Game Directory", "Chọn thư mục game Global"),
    ("🔄 Reload Global", "🔄 Tải lại Global"),
    (
        "Reload Global manifest and master databases from disk",
        "Tải lại manifest và cơ sở dữ liệu master của Global từ đĩa",
    ),
    (
        "No Global directory configured.",
        "Chưa cấu hình thư mục game Global.",
    ),
    ("Scanning…", "Đang quét…"),
    ("Loading Global database…", "Đang nạp database Global…"),
    (
        "✓ Global: {} audio entries · {} live songs, {} jukebox tracks",
        "✓ Global: {} mục âm thanh · {} bài live, {} bài jukebox",
    ),
    ("JP version", "Phiên bản JP"),
    ("JP directory", "Thư mục JP"),
    (
        "Path to the Persistent directory containing dat, meta, and master folders for JP (Steam or DMM).",
        "Đường dẫn đến thư mục Persistent chứa dat, meta và master của bản JP (Steam hoặc DMM).",
    ),
    (
        "e.g. C:\\...\\UmamusumePrettyDerby_Jpn or DMM Persistent",
        "vd: C:\\...\\UmamusumePrettyDerby_Jpn hoặc DMM Persistent",
    ),
    (
        "Select JP game directory via folder picker",
        "Chọn thư mục game JP bằng cửa sổ chọn thư mục",
    ),
    ("Select JP Game Directory", "Chọn thư mục game JP"),
    ("🔄 Reload JP", "🔄 Tải lại JP"),
    (
        "Reload JP manifest and master databases from disk",
        "Tải lại manifest và cơ sở dữ liệu master của JP từ đĩa",
    ),
    (
        "No JP directory configured.",
        "Chưa cấu hình thư mục game JP.",
    ),
    ("Loading JP database…", "Đang nạp database JP…"),
    (
        "✓ JP: {} audio entries · {} live songs, {} jukebox tracks",
        "✓ JP: {} mục âm thanh · {} bài live, {} bài jukebox",
    ),
    ("Game discovery", "Tìm kiếm bản cài game"),
    (
        "Scan for installed Global and JP game clients.",
        "Quét tìm các bản cài đặt game Global và JP trên máy.",
    ),
    (
        "Click to assign as game directory",
        "Nhấn để chọn làm thư mục game",
    ),
    // Phase 7 — Uma Character Avatars in Live Stage
    ("Performer avatar", "Ảnh đại diện ca sĩ"),
    ("Main vocal avatar", "Ảnh đại diện ca sĩ chính"),
    ("★ Center", "★ Vị trí trung tâm"),
    ("STAGE PERFORMER LINEUP", "ĐỘI HÌNH BIỂU DIỄN SÂN KHẤU"),
    (
        "Uma avatar (game asset or packaged art)",
        "Ảnh đại diện Uma (ảnh từ game hoặc tài nguyên đóng gói)",
    ),
    // Phase 8 — In-game Full Resource Download Advisory Notice
    (
        "Recommendation: Open the {} game and download full in-game data so audio tracks, vocals, lyrics, and jacket/character art are fully available.",
        "Khuyến nghị: Hãy mở game {} và tải đầy đủ tài nguyên trong game để bài hát, giọng ca sĩ, lời hát và ảnh bìa/avatar hoạt động đầy đủ nhất.",
    ),
    (
        "Missing game assets may cause certain songs, vocals, lyrics, or jacket/character images to be unavailable in Music.",
        "Nếu thiếu tài nguyên game, một số bài hát, giọng ca sĩ, lời hát hoặc ảnh bìa/avatar có thể không hiển thị hoặc không phát được trong Âm thanh game.",
    ),
    // Phase 9 — Vocal Mode & Singer Selection in Live Stage
    ("Vocal mode:", "Chế độ giọng hát:"),
    ("🎤 Solo Vocal", "🎤 Đơn ca"),
    ("👥 Full Cast (Auto Part)", "👥 Hợp ca (Auto Part)"),
    ("🎼 Instrumental", "🎼 Nhạc nền"),
    ("⚙ Custom", "⚙ Tùy chỉnh"),
    (
        "Choose one Uma as exclusive main singer; mute all other slots",
        "Chọn một Uma làm người hát chính duy nhất; tắt tất cả các vị trí khác",
    ),
    (
        "Perform original stage score triggers across all slots",
        "Biểu diễn theo kịch bản phân bè gốc của sân khấu trên tất cả vị trí",
    ),
    (
        "Mute all singers and play instrumental accompaniment only",
        "Tắt toàn bộ ca sĩ và chỉ phát nhạc đệm không lời",
    ),
    ("Solo singer:", "Ca sĩ đơn ca:"),
    ("🎤 Set as Solo", "🎤 Chọn đơn ca"),
    ("🎤 Solo [Active]", "🎤 Đơn ca [Bật]"),
    (
        "Currently the exclusive main singer. Click to mute and switch to Instrumental.",
        "Đang là ca sĩ đơn ca duy nhất. Bấm để tắt và chuyển sang Nhạc nền.",
    ),
    (
        "Set this performer as the exclusive main singer; mutes all other stage slots.",
        "Chọn ca sĩ này làm người hát chính duy nhất; tắt toàn bộ các vị trí sân khấu khác.",
    ),
    (
        "🎼 Instrumental mode (No vocal selected). Playing live will play the accompaniment only. Select [🎤 Solo Vocal] or [👥 Full Cast] to enable singing.",
        "🎼 Chế độ Nhạc nền (Chưa chọn ca sĩ). Khi phát sẽ chỉ có nhạc đệm không lời. Chọn [🎤 Đơn ca] hoặc [👥 Hợp ca] để bật giọng hát.",
    ),
    (
        "🎤 Solo Vocal: {} (Slot {}). All other stage slots are muted for clean single-singer playback.",
        "🎤 Đơn ca: {} (Vị trí {}). Toàn bộ các vị trí khác đã được tắt để phát đơn ca chuẩn xác.",
    ),
    (
        "👥 Full Cast (Auto Part): All performers sing according to original stage part triggers.",
        "👥 Hợp ca (Auto Part): Tất cả ca sĩ biểu diễn theo kịch bản phân bè gốc của sân khấu.",
    ),
    (
        "ℹ This live song has no vocal data in game files (Instrumental only).",
        "ℹ Bài hát này không có dữ liệu giọng hát trong game (Chỉ phát nhạc nền).",
    ),
    (
        "⚠ Voice file is not downloaded in game yet.",
        "⚠ Tệp giọng hát chưa được tải về trong game.",
    ),
    ("#{0} • {1} members", "#{0} • {1} ca sĩ"),
    ("(Slot {})", "(Vị trí {})"),
    ("★ Slot {} (Center)", "★ Vị trí {} (Trung tâm)"),
    ("🎤 Main vocal", "🎤 Hát chính"),
    ("{0} - {1} (Slot {2})", "{0} - {1} (Vị trí {2})"),
    ("{0} - {1} (Slot {2}) · {3}", "{0} - {1} (Vị trí {2}) · {3}"),
    ("Active voices: {}", "Giọng đang hát: {}"),
    ("Exported: {}", "Đã xuất: {}"),
    ("Track", "Bản nhạc"),
    // Phase 3 — Music Beta notice & labeling
    ("Beta", "Beta"),
    ("(Beta)", "(Beta)"),
    ("🎵 Music (Beta)", "🎵 Âm nhạc (Beta)"),
    ("🎵 Music Feature (Beta)", "🎵 Tính năng Âm nhạc (Beta)"),
    (
        "The Music and Game audio player is currently in Beta. It reads local game files offline to provide audio preview, Jukebox songs, and multi-track Live performances.",
        "Trình phát âm thanh game và tính năng Âm nhạc hiện đang trong giai đoạn Beta. Tính năng này đọc các tệp game cục bộ ngoại tuyến để phát thử âm thanh, bài hát Jukebox và trình diễn Live đa kênh.",
    ),
    (
        "Compatibility and available tracks depend on your locally downloaded game assets. You can always review this information in Settings → Game audio.",
        "Khả năng tương thích và các bản nhạc khả dụng phụ thuộc vào tài nguyên game bạn đã tải về cục bộ. Bạn luôn có thể xem lại thông tin này trong Cài đặt → Âm thanh game.",
    ),
    (
        "The Music and Game audio player is currently in Beta. It reads local game files offline to provide audio preview, Jukebox songs, and multi-track Live performances. Compatibility and available tracks depend on your locally downloaded game assets.",
        "Trình phát âm thanh game và tính năng Âm nhạc hiện đang trong giai đoạn Beta. Tính năng này đọc các tệp game cục bộ ngoại tuyến để phát thử âm thanh, bài hát Jukebox và trình diễn Live đa kênh. Khả năng tương thích và các bản nhạc khả dụng phụ thuộc vào tài nguyên game bạn đã tải về cục bộ.",
    ),
    ("Got it", "Đã hiểu"),
    // Phase 6 — Music Live overall vocal volume & startup default
    ("Vocal volume:", "Âm lượng vocal:"),
    (
        "Overall volume for all live performers; scales per-slot volumes",
        "Âm lượng chung cho toàn bộ ca sĩ Live; tỷ lệ với âm lượng từng vị trí",
    ),
];
