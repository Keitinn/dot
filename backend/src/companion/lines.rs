//! Lines dot says when the popover opens (design/gdd/companion.md R3 / R4).
//! The text lives in `lines.json`; this module only picks one, or none.

use std::sync::OnceLock;
use rand::{seq::IndexedRandom, Rng};
use serde::Deserialize;
use super::consts::*;
use super::mood::{Mood, Need};
use super::state::LineHistory;

/// R3 categories, in priority order. `battery` and focus lines are Standard.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Category {
    GreetFirstToday,
    MoodConcerned,
    MissedYou,
    ReturnAfterAway,
    BondTier,
    TimeOfDay,
    Mood,
}

impl Category {
    pub const PRIORITY: [Category; 7] = [
        Category::GreetFirstToday,
        Category::MoodConcerned,
        Category::MissedYou,
        Category::ReturnAfterAway,
        Category::BondTier,
        Category::TimeOfDay,
        Category::Mood,
    ];

    /// The key in `LineHistory::category_last_at`.
    pub fn as_str(self) -> &'static str {
        match self {
            Category::GreetFirstToday => "greet_first_today",
            Category::MoodConcerned => "mood_concerned",
            Category::MissedYou => "missed_you",
            Category::ReturnAfterAway => "return_after_away",
            Category::BondTier => "bond_tier",
            Category::TimeOfDay => "time_of_day",
            Category::Mood => "mood",
        }
    }
}

/// Part of the local day.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Part {
    /// 05–10
    Morning,
    /// 11–16
    Day,
    /// 17–21
    Evening,
    /// 22–04
    Night,
}

impl Part {
    pub fn of_hour(hour: u32) -> Part {
        match hour {
            5..=10 => Part::Morning,
            11..=16 => Part::Day,
            17..=21 => Part::Evening,
            _ => Part::Night,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Line {
    /// Stable; saved in the history.
    pub id: String,
    pub category: Category,
    /// Upper case, rows split by `\n`.
    pub text: String,
    /// Lowest Bond tier that unlocks the line (0 = from the start). For
    /// `bond_tier` lines, the tier they celebrate.
    #[serde(default)]
    pub min_tier: u16,
    /// Only said in this part of the day.
    #[serde(default)]
    pub part: Option<Part>,
    /// `mood_concerned`: only said about this need.
    #[serde(default)]
    pub need: Option<Need>,
    /// `mood`: only said in this mood.
    #[serde(default)]
    pub mood: Option<Mood>,
}

const LINES_JSON: &str = include_str!("lines.json");

/// Every line, parsed once. The data is checked by the tests below.
pub fn all() -> &'static [Line] {
    static LINES: OnceLock<Vec<Line>> = OnceLock::new();
    LINES.get_or_init(|| serde_json::from_str(LINES_JSON).expect("lines.json is valid"))
}

/// What the choice of a line depends on.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct LineContext {
    /// Unix seconds.
    pub now: i64,
    /// Local hour, 0–23.
    pub hour: u32,
    pub mood: Mood,
    /// What `concerned` is about, when the mood is `concerned`.
    pub concerned_need: Option<Need>,
    /// The first manual open of the local day.
    pub first_today: bool,
    /// Seconds since the previous manual open; `None` if never opened.
    pub secs_since_open: Option<i64>,
    /// Highest Bond tier reached (0, 100, 300, 600 or 1000).
    pub tier: u16,
    /// `tier` was reached and has not been marked by a `bond_tier` line yet.
    pub tier_unsaid: bool,
}

/// The line to say, or `None` for silence: the first category in priority
/// order whose condition holds, that is off cooldown and that has a line not
/// said recently. `time_of_day` and `mood` speak with chance `speak_p`; when
/// they stay quiet, nothing lower is tried.
pub fn select<'a, R: Rng + ?Sized>(
    lines: &'a [Line],
    ctx: &LineContext,
    history: &LineHistory,
    speak_p: f64,
    rng: &mut R,
) -> Option<&'a Line> {
    for category in Category::PRIORITY {
        if !applies(category, ctx) || !cooled_down(category, ctx, history) {
            continue;
        }
        let pool = candidates(lines, category, ctx, history);
        if pool.is_empty() {
            continue;
        }
        if is_optional(category) && !rng.random_bool(speak_p.clamp(0.0, 1.0)) {
            return None;
        }
        return pool.choose(rng).copied();
    }
    None
}

/// The R3 condition of `category`.
fn applies(category: Category, ctx: &LineContext) -> bool {
    let away_at_least = |hours: i64| ctx.secs_since_open.is_some_and(|secs| secs >= hours * 3600);
    match category {
        Category::GreetFirstToday => ctx.first_today,
        Category::MoodConcerned => ctx.mood == Mood::Concerned && ctx.concerned_need.is_some(),
        Category::MissedYou => away_at_least(MISSED_YOU_H),
        Category::ReturnAfterAway => away_at_least(AWAY_RETURN_H),
        Category::BondTier => ctx.tier_unsaid && ctx.tier > 0,
        Category::TimeOfDay => true,
        Category::Mood => ctx.mood != Mood::Concerned,
    }
}

/// `time_of_day` and `mood` speak only sometimes.
fn is_optional(category: Category) -> bool {
    matches!(category, Category::TimeOfDay | Category::Mood)
}

/// Each category once in LINE_CATEGORY_COOLDOWN_H, except the greeting
/// (once a day by its condition) and `concerned`, which waits
/// CONCERNED_VALVE_H per need instead.
fn cooled_down(category: Category, ctx: &LineContext, history: &LineHistory) -> bool {
    match category {
        Category::GreetFirstToday => true,
        Category::MoodConcerned => {
            let need = ctx.concerned_need.map_or("", |n| n.as_str());
            waited(history.concerned_last_at.get(need), ctx.now, CONCERNED_VALVE_H)
        }
        _ => waited(history.category_last_at.get(category.as_str()), ctx.now, LINE_CATEGORY_COOLDOWN_H),
    }
}

/// `hours` have passed since `last`. A `last` in the future (the clock was
/// set back) counts as passed, so a category is never silenced for good.
fn waited(last: Option<&i64>, now: i64, hours: i64) -> bool {
    last.is_none_or(|&last| last > now || now - last >= hours * 3600)
}

/// The lines of `category` that fit `ctx` and were not said recently.
pub fn candidates<'a>(
    lines: &'a [Line],
    category: Category,
    ctx: &LineContext,
    history: &LineHistory,
) -> Vec<&'a Line> {
    let part = Part::of_hour(ctx.hour);
    lines
        .iter()
        .filter(|l| l.category == category)
        .filter(|l| match category {
            // Each tier has its own lines
            Category::BondTier => l.min_tier == ctx.tier,
            _ => l.min_tier <= ctx.tier,
        })
        .filter(|l| l.part.is_none_or(|p| p == part))
        .filter(|l| l.need.is_none_or(|n| Some(n) == ctx.concerned_need))
        .filter(|l| l.mood.is_none_or(|m| m == ctx.mood))
        .filter(|l| !history.recent_ids.contains(&l.id))
        .collect()
}

/// Records that `line` was said at `ctx.now`.
pub fn remember(history: &mut LineHistory, line: &Line, ctx: &LineContext) {
    history.recent_ids.push(line.id.clone());
    let excess = history.recent_ids.len().saturating_sub(LINE_HISTORY);
    history.recent_ids.drain(..excess);
    history.category_last_at.insert(line.category.as_str().to_string(), ctx.now);
    if line.category == Category::MoodConcerned {
        if let Some(need) = ctx.concerned_need {
            history.concerned_last_at.insert(need.as_str().to_string(), ctx.now);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rand::{rngs::StdRng, SeedableRng};
    use std::collections::HashSet;

    const H: i64 = 3600;
    const T0: i64 = 1_800_000_000;

    fn rng() -> StdRng {
        StdRng::seed_from_u64(7)
    }

    /// Noon, cheerful, not the first open, opened 1 hour ago, tier 0.
    fn ctx() -> LineContext {
        LineContext {
            now: T0,
            hour: 12,
            mood: Mood::Cheerful,
            concerned_need: None,
            first_today: false,
            secs_since_open: Some(H),
            tier: 0,
            tier_unsaid: false,
        }
    }

    fn line(id: &str, category: Category) -> Line {
        Line {
            id: id.to_string(),
            category,
            text: id.to_uppercase(),
            min_tier: 0,
            part: None,
            need: None,
            mood: None,
        }
    }

    /// Two plain lines per category, tagged so they fit `ctx()` (and hunger /
    /// energy for concerned), plus tiered lines.
    fn sample() -> &'static [Line] {
        static SAMPLE: OnceLock<Vec<Line>> = OnceLock::new();
        SAMPLE.get_or_init(build_sample)
    }

    fn build_sample() -> Vec<Line> {
        let mut lines = Vec::new();
        for c in Category::PRIORITY {
            for n in 1..=2 {
                let mut l = line(&format!("{}_{}", c.as_str(), n), c);
                match c {
                    Category::MoodConcerned => l.need = Some(Need::Hunger),
                    Category::BondTier => l.min_tier = 100,
                    Category::Mood => l.mood = Some(Mood::Cheerful),
                    _ => {}
                }
                lines.push(l);
            }
        }
        lines.push(Line { need: Some(Need::Energy), ..line("concerned_energy", Category::MoodConcerned) });
        lines.push(Line { min_tier: 300, ..line("bond_300", Category::BondTier) });
        lines.push(Line { min_tier: 300, ..line("greet_300", Category::GreetFirstToday) });
        lines.push(Line { part: Some(Part::Night), ..line("tod_night", Category::TimeOfDay) });
        lines.push(Line { mood: Some(Mood::Sleepy), ..line("mood_sleepy", Category::Mood) });
        lines
    }

    fn pick(ctx: &LineContext, history: &LineHistory, p: f64) -> Option<Category> {
        select(sample(), ctx, history, p, &mut rng()).map(|l| l.category)
    }

    fn said(history: &mut LineHistory, category: Category, at: i64) {
        history.category_last_at.insert(category.as_str().to_string(), at);
    }

    // Priority

    #[test]
    fn categories_go_in_priority_order() {
        let mut c = LineContext {
            first_today: true,
            mood: Mood::Concerned,
            concerned_need: Some(Need::Hunger),
            secs_since_open: Some(30 * H),
            tier: 100,
            tier_unsaid: true,
            ..ctx()
        };
        let empty = LineHistory::default();
        assert_eq!(pick(&c, &empty, 1.0), Some(Category::GreetFirstToday));
        c.first_today = false;
        assert_eq!(pick(&c, &empty, 1.0), Some(Category::MoodConcerned));
        c.mood = Mood::MissedYou;
        c.concerned_need = None;
        assert_eq!(pick(&c, &empty, 1.0), Some(Category::MissedYou));
        c.secs_since_open = Some(5 * H);
        assert_eq!(pick(&c, &empty, 1.0), Some(Category::ReturnAfterAway));
        c.secs_since_open = Some(H);
        assert_eq!(pick(&c, &empty, 1.0), Some(Category::BondTier));
        c.tier_unsaid = false;
        c.mood = Mood::Cheerful;
        assert_eq!(pick(&c, &empty, 1.0), Some(Category::TimeOfDay));
        let mut history = LineHistory::default();
        said(&mut history, Category::TimeOfDay, T0 - H);
        assert_eq!(pick(&c, &history, 1.0), Some(Category::Mood));
        said(&mut history, Category::Mood, T0 - H);
        assert_eq!(pick(&c, &history, 1.0), None);
    }

    #[test]
    fn greet_first_today_ignores_the_category_cooldown() {
        let mut history = LineHistory::default();
        said(&mut history, Category::GreetFirstToday, T0 - 60);
        let c = LineContext { first_today: true, ..ctx() };
        assert_eq!(pick(&c, &history, 0.0), Some(Category::GreetFirstToday));
    }

    #[test]
    fn greet_needs_the_first_open_of_the_day() {
        assert_eq!(pick(&ctx(), &LineHistory::default(), 0.0), None);
    }

    // mood_concerned: per need, every 6 hours

    #[test]
    fn concerned_about_the_same_need_waits_6_hours() {
        let c = LineContext { mood: Mood::Concerned, concerned_need: Some(Need::Hunger), ..ctx() };
        let mut history = LineHistory::default();
        let first = select(sample(), &c, &history, 0.0, &mut rng()).unwrap();
        assert_eq!(first.category, Category::MoodConcerned);
        remember(&mut history, first, &c);

        let later = LineContext { now: T0 + 6 * H - 1, ..c };
        assert_eq!(pick(&later, &history, 0.0), None);
        let later = LineContext { now: T0 + 6 * H, ..c };
        assert_eq!(pick(&later, &history, 0.0), Some(Category::MoodConcerned));
    }

    #[test]
    fn concerned_about_another_need_is_not_held_back() {
        let c = LineContext { mood: Mood::Concerned, concerned_need: Some(Need::Hunger), ..ctx() };
        let mut history = LineHistory::default();
        let first = select(sample(), &c, &history, 0.0, &mut rng()).unwrap();
        remember(&mut history, first, &c);

        let energy = LineContext { now: T0 + H, concerned_need: Some(Need::Energy), ..c };
        let line = select(sample(), &energy, &history, 0.0, &mut rng()).unwrap();
        assert_eq!(line.id, "concerned_energy");
    }

    #[test]
    fn concerned_lines_fit_the_need() {
        let c = LineContext { mood: Mood::Concerned, concerned_need: Some(Need::Energy), ..ctx() };
        let ids: Vec<_> = candidates(sample(), Category::MoodConcerned, &c, &LineHistory::default())
            .iter()
            .map(|l| l.id.as_str())
            .collect();
        assert_eq!(ids, vec!["concerned_energy"]);
    }

    #[test]
    fn concerned_lines_need_the_concerned_mood() {
        let c = LineContext { concerned_need: Some(Need::Hunger), ..ctx() };
        assert_eq!(pick(&c, &LineHistory::default(), 0.0), None);
    }

    // 24-hour category cooldown

    #[test]
    fn a_category_speaks_once_in_24_hours() {
        let c = LineContext { secs_since_open: Some(5 * H), ..ctx() };
        let mut history = LineHistory::default();
        let first = select(sample(), &c, &history, 0.0, &mut rng()).unwrap();
        assert_eq!(first.category, Category::ReturnAfterAway);
        remember(&mut history, first, &c);

        let nine_hours = LineContext { now: T0 + 9 * H, ..c };
        assert_eq!(pick(&nine_hours, &history, 0.0), None);
        let a_day = LineContext { now: T0 + 24 * H, ..c };
        assert_eq!(pick(&a_day, &history, 0.0), Some(Category::ReturnAfterAway));
    }

    #[test]
    fn a_clock_set_back_does_not_silence_a_category_for_good() {
        let mut history = LineHistory::default();
        said(&mut history, Category::ReturnAfterAway, T0 + 100 * H);
        let c = LineContext { secs_since_open: Some(5 * H), ..ctx() };
        assert_eq!(pick(&c, &history, 0.0), Some(Category::ReturnAfterAway));
    }

    // History of 20

    #[test]
    fn remember_keeps_the_last_20_ids_and_the_time() {
        let mut history = LineHistory::default();
        let c = ctx();
        for n in 0..25 {
            remember(&mut history, &line(&format!("l{}", n), Category::Mood), &c);
        }
        assert_eq!(history.recent_ids.len(), LINE_HISTORY);
        assert_eq!(history.recent_ids.first().map(String::as_str), Some("l5"));
        assert_eq!(history.recent_ids.last().map(String::as_str), Some("l24"));
        assert_eq!(history.category_last_at.get("mood"), Some(&T0));
    }

    #[test]
    fn remember_marks_the_need_for_concerned_lines() {
        let mut history = LineHistory::default();
        let c = LineContext { mood: Mood::Concerned, concerned_need: Some(Need::Energy), ..ctx() };
        remember(&mut history, &line("x", Category::MoodConcerned), &c);
        assert_eq!(history.concerned_last_at.get("energy"), Some(&T0));
    }

    #[test]
    fn a_line_in_the_history_is_not_said() {
        let c = LineContext { secs_since_open: Some(5 * H), ..ctx() };
        let mut history = LineHistory::default();
        history.recent_ids.push("return_after_away_1".to_string());
        for seed in 0..20 {
            let l = select(sample(), &c, &history, 0.0, &mut StdRng::seed_from_u64(seed)).unwrap();
            assert_eq!(l.id, "return_after_away_2");
        }
    }

    #[test]
    fn a_category_with_every_line_in_the_history_is_skipped() {
        let c = LineContext { secs_since_open: Some(5 * H), tier: 100, tier_unsaid: true, ..ctx() };
        let mut history = LineHistory::default();
        history.recent_ids.extend(["return_after_away_1".to_string(), "return_after_away_2".to_string()]);
        assert_eq!(pick(&c, &history, 0.0), Some(Category::BondTier));
    }

    #[test]
    fn a_hundred_picks_never_repeat_the_last_20() {
        let lines = all();
        let mut history = LineHistory::default();
        let mut r = rng();
        let mut spoken = 0;
        for n in 0..100i64 {
            let c = LineContext {
                now: T0 + n * 25 * H,
                hour: (n * 7 % 24) as u32,
                first_today: n % 3 == 0,
                secs_since_open: Some((n % 5) * 8 * H),
                mood: [Mood::Calm, Mood::Cheerful, Mood::Sleepy][(n % 3) as usize],
                tier: 1000,
                ..ctx()
            };
            if let Some(l) = select(lines, &c, &history, 1.0, &mut r) {
                assert!(!history.recent_ids.contains(&l.id), "{} repeated", l.id);
                remember(&mut history, l, &c);
                spoken += 1;
            }
        }
        assert!(spoken > 50, "only {} lines", spoken);
    }

    // Chance

    #[test]
    fn chance_0_never_speaks_and_1_always_speaks() {
        for seed in 0..50 {
            let mut r = StdRng::seed_from_u64(seed);
            assert!(select(sample(), &ctx(), &LineHistory::default(), 0.0, &mut r).is_none());
            let mut r = StdRng::seed_from_u64(seed);
            assert_eq!(
                select(sample(), &ctx(), &LineHistory::default(), 1.0, &mut r).map(|l| l.category),
                Some(Category::TimeOfDay)
            );
        }
    }

    #[test]
    fn a_quiet_time_of_day_does_not_fall_through_to_mood() {
        // With chance 0 time_of_day stays quiet; mood is not tried after it
        assert_eq!(pick(&ctx(), &LineHistory::default(), 0.0), None);
        // Once time_of_day is on cooldown, mood is reached and rolls for itself
        let mut history = LineHistory::default();
        said(&mut history, Category::TimeOfDay, T0 - H);
        assert_eq!(pick(&ctx(), &history, 0.0), None);
        assert_eq!(pick(&ctx(), &history, 1.0), Some(Category::Mood));
    }

    #[test]
    fn chance_one_half_speaks_about_half_the_time() {
        let mut r = rng();
        let spoke = (0..1000)
            .filter(|_| select(sample(), &ctx(), &LineHistory::default(), LINE_SPEAK_P, &mut r).is_some())
            .count();
        assert!((450..=550).contains(&spoke), "{} of 1000", spoke);
    }

    #[test]
    fn the_same_seed_picks_the_same_lines() {
        let run = |seed| {
            let mut r = StdRng::seed_from_u64(seed);
            (0..30)
                .map(|n| {
                    let c = LineContext { hour: n % 24, ..ctx() };
                    select(all(), &c, &LineHistory::default(), 0.5, &mut r).map(|l| l.id.clone())
                })
                .collect::<Vec<_>>()
        };
        assert_eq!(run(42), run(42));
        assert!(run(42).iter().any(Option::is_some));
    }

    #[test]
    fn nothing_fits_means_silence() {
        assert_eq!(select(&[], &ctx(), &LineHistory::default(), 1.0, &mut rng()), None);
        let c = LineContext { secs_since_open: None, ..ctx() };
        let only_away = vec![line("a", Category::ReturnAfterAway)];
        assert_eq!(select(&only_away, &c, &LineHistory::default(), 1.0, &mut rng()), None);
    }

    // Bond tier

    #[test]
    fn bond_tier_needs_an_unsaid_tier() {
        let c = LineContext { tier: 100, ..ctx() };
        assert_ne!(pick(&c, &LineHistory::default(), 0.0), Some(Category::BondTier));
        let c = LineContext { tier_unsaid: true, ..c };
        assert_eq!(pick(&c, &LineHistory::default(), 0.0), Some(Category::BondTier));
    }

    #[test]
    fn bond_tier_lines_are_for_the_tier_reached() {
        let c = LineContext { tier: 300, tier_unsaid: true, ..ctx() };
        let ids: Vec<_> = candidates(sample(), Category::BondTier, &c, &LineHistory::default())
            .iter()
            .map(|l| l.id.as_str())
            .collect();
        assert_eq!(ids, vec!["bond_300"]);
    }

    #[test]
    fn tier_0_has_no_bond_tier_line() {
        let c = LineContext { tier: 0, tier_unsaid: true, ..ctx() };
        assert!(candidates(sample(), Category::BondTier, &c, &LineHistory::default()).is_empty());
    }

    #[test]
    fn min_tier_unlocks_lines() {
        let greet = |tier| {
            let c = LineContext { tier, first_today: true, ..ctx() };
            candidates(sample(), Category::GreetFirstToday, &c, &LineHistory::default())
                .iter()
                .map(|l| l.id.clone())
                .collect::<Vec<_>>()
        };
        assert!(!greet(0).contains(&"greet_300".to_string()));
        assert!(!greet(100).contains(&"greet_300".to_string()));
        assert!(greet(300).contains(&"greet_300".to_string()));
        assert!(greet(1000).contains(&"greet_300".to_string()));
    }

    #[test]
    fn tags_filter_by_part_and_mood() {
        let night = LineContext { hour: 23, mood: Mood::Sleepy, ..ctx() };
        let tod: Vec<_> = candidates(sample(), Category::TimeOfDay, &night, &LineHistory::default())
            .iter()
            .map(|l| l.id.as_str())
            .collect();
        assert!(tod.contains(&"tod_night"));
        let mood: Vec<_> = candidates(sample(), Category::Mood, &night, &LineHistory::default())
            .iter()
            .map(|l| l.id.as_str())
            .collect();
        assert_eq!(mood, vec!["mood_sleepy"]);
        assert!(!candidates(sample(), Category::TimeOfDay, &ctx(), &LineHistory::default())
            .iter()
            .any(|l| l.id == "tod_night"));
    }

    #[test]
    fn mood_lines_are_not_said_while_concerned() {
        let mut l = line("mood_concerned_tag", Category::Mood);
        l.mood = Some(Mood::Concerned);
        let c = LineContext { mood: Mood::Concerned, concerned_need: None, ..ctx() };
        let mut history = LineHistory::default();
        said(&mut history, Category::TimeOfDay, T0);
        assert_eq!(select(&[l], &c, &history, 1.0, &mut rng()), None);
    }

    // R4: time since the last open

    #[test]
    fn away_3h59m_says_nothing_about_coming_back() {
        let c = LineContext { secs_since_open: Some(4 * H - 60), ..ctx() };
        assert_eq!(pick(&c, &LineHistory::default(), 0.0), None);
    }

    #[test]
    fn away_4h_is_return_after_away() {
        let c = LineContext { secs_since_open: Some(4 * H), ..ctx() };
        assert_eq!(pick(&c, &LineHistory::default(), 0.0), Some(Category::ReturnAfterAway));
    }

    #[test]
    fn away_24h_is_missed_you() {
        let c = LineContext { secs_since_open: Some(24 * H), mood: Mood::MissedYou, ..ctx() };
        assert_eq!(pick(&c, &LineHistory::default(), 0.0), Some(Category::MissedYou));
    }

    #[test]
    fn parts_of_the_day() {
        let parts: Vec<_> = (0..24).map(Part::of_hour).collect();
        use Part::*;
        assert_eq!(
            parts,
            vec![
                Night, Night, Night, Night, Night, Morning, Morning, Morning, Morning, Morning, Morning,
                Day, Day, Day, Day, Day, Day, Evening, Evening, Evening, Evening, Evening, Night, Night,
            ]
        );
    }

    // The data in lines.json

    /// Phrases that guilt, command, rush or judge (design 0.3 anti-pillars and
    /// Player Fantasy), matched as whole words.
    const FORBIDDEN: &[&str] = &[
        "LONELY", "ALONE", "MISSED YOU", "MISS YOU", "MISSING YOU", "WHERE WERE YOU",
        "WHERE HAVE YOU", "WHERE DID YOU", "LEFT ME", "ABANDON", "ABANDONED", "FORGOT",
        "FORGOTTEN", "FORGET", "WAITED", "FINALLY", "AT LAST", "SO LONG", "WHY", "SAD",
        "HURRY", "YOU MUST", "YOU SHOULD", "YOU NEED", "YOU HAVE TO", "DON'T", "REMEMBER TO",
        "PLEASE", "FEED ME", "PLAY WITH ME", "PET ME", "GO TO BED", "GO TO SLEEP",
        "TAKE A BREAK", "GET BACK", "WORK", "PRODUCTIVE", "LAZY", "GOOD JOB", "WELL DONE",
    ];

    fn words(text: &str) -> String {
        let spaced: String = text
            .chars()
            .map(|c| if c.is_ascii_uppercase() || c.is_ascii_digit() || c == '\'' { c } else { ' ' })
            .collect();
        format!(" {} ", spaced.split_whitespace().collect::<Vec<_>>().join(" "))
    }

    #[test]
    fn data_has_at_least_40_lines_and_6_per_category() {
        let lines = all();
        assert!(lines.len() >= LINES_MIN, "{} lines", lines.len());
        for c in Category::PRIORITY {
            let n = lines.iter().filter(|l| l.category == c).count();
            assert!(n >= LINES_PER_CATEGORY_MIN, "{}: {} lines", c.as_str(), n);
        }
    }

    #[test]
    fn data_lines_fit_the_bubble() {
        for l in all() {
            let rows: Vec<_> = l.text.split('\n').collect();
            assert!(rows.len() <= LINE_MAX_ROWS, "{}: {} rows", l.id, rows.len());
            assert!(rows.iter().all(|r| !r.trim().is_empty()), "{}: empty row", l.id);
            let chars = l.text.chars().filter(|&c| c != '\n').count();
            assert!(chars <= LINE_MAX_CHARS, "{}: {} chars", l.id, chars);
        }
    }

    #[test]
    fn data_is_plain_upper_case_without_exclamations() {
        for l in all() {
            assert!(
                l.text.chars().all(|c| c.is_ascii_uppercase()
                    || c.is_ascii_digit()
                    || " .,?'-\n".contains(c)),
                "{}: {:?}",
                l.id,
                l.text
            );
        }
    }

    #[test]
    fn data_ids_are_unique_and_stable_looking() {
        let mut seen = HashSet::new();
        for l in all() {
            assert!(
                !l.id.is_empty() && l.id.chars().all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '_'),
                "bad id {:?}",
                l.id
            );
            assert!(seen.insert(l.id.as_str()), "duplicate id {}", l.id);
        }
    }

    #[test]
    fn data_has_no_forbidden_phrases() {
        for l in all() {
            let text = words(&l.text);
            for phrase in FORBIDDEN {
                assert!(!text.contains(&format!(" {} ", phrase)), "{}: {:?} has {}", l.id, l.text, phrase);
            }
        }
    }

    #[test]
    fn forbidden_phrase_check_matches_whole_words() {
        assert!(words("I MISSED YOU...").contains(" MISSED YOU "));
        assert!(!words("SWORK").contains(" WORK "));
        assert!(words("DON'T FORGET.").contains(" DON'T "));
    }

    #[test]
    fn data_tags_fit_their_category() {
        for l in all() {
            assert!(l.min_tier == 0 || BOND_TIERS.contains(&l.min_tier), "{}: tier {}", l.id, l.min_tier);
            match l.category {
                Category::MoodConcerned => assert!(l.need.is_some(), "{} lacks need", l.id),
                _ => assert!(l.need.is_none(), "{} has need", l.id),
            }
            match l.category {
                Category::Mood => assert!(
                    matches!(l.mood, Some(Mood::Sleepy | Mood::Calm | Mood::Cheerful)),
                    "{}: mood {:?}",
                    l.id,
                    l.mood
                ),
                _ => assert!(l.mood.is_none(), "{} has mood", l.id),
            }
            match l.category {
                Category::TimeOfDay => assert!(l.part.is_some(), "{} lacks part", l.id),
                Category::GreetFirstToday => {}
                _ => assert!(l.part.is_none(), "{} has part", l.id),
            }
            if l.category == Category::BondTier {
                assert!(l.min_tier > 0, "{}: bond_tier at tier 0", l.id);
            }
        }
    }

    #[test]
    fn data_mood_names_are_spelled_right() {
        // An unknown mood would read as calm, so check the raw JSON
        let raw: Vec<serde_json::Value> = serde_json::from_str(LINES_JSON).unwrap();
        for v in raw {
            if let Some(mood) = v.get("mood") {
                assert!(["sleepy", "calm", "cheerful"].contains(&mood.as_str().unwrap()), "{}", v);
            }
        }
    }

    #[test]
    fn data_covers_every_situation_from_tier_0() {
        let lines = all();
        let empty = LineHistory::default();
        for hour in 0..24 {
            let c = LineContext { hour, first_today: true, ..ctx() };
            assert!(!candidates(lines, Category::GreetFirstToday, &c, &empty).is_empty(), "greet {}h", hour);
            assert!(!candidates(lines, Category::TimeOfDay, &c, &empty).is_empty(), "tod {}h", hour);
        }
        for need in [Need::Hunger, Need::Happiness, Need::Energy, Need::Health] {
            let c = LineContext { mood: Mood::Concerned, concerned_need: Some(need), ..ctx() };
            assert!(candidates(lines, Category::MoodConcerned, &c, &empty).len() >= 2, "{:?}", need);
        }
        for mood in [Mood::Sleepy, Mood::Calm, Mood::Cheerful] {
            let c = LineContext { mood, ..ctx() };
            assert!(!candidates(lines, Category::Mood, &c, &empty).is_empty(), "{:?}", mood);
        }
        for category in [Category::MissedYou, Category::ReturnAfterAway] {
            assert!(candidates(lines, category, &ctx(), &empty).len() >= 6, "{:?}", category);
        }
        for tier in BOND_TIERS {
            let c = LineContext { tier, tier_unsaid: true, ..ctx() };
            assert!(!candidates(lines, Category::BondTier, &c, &empty).is_empty(), "tier {}", tier);
        }
    }
}
