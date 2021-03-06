#![allow(clippy::collapsible_else_if)]

pub mod quad_storage;

use ::rand::prelude::*;
use serde::*;
use std::collections::BTreeMap;
use std::collections::BTreeSet;
// use eframe::egui_web;

type Rand = rand_pcg::Pcg64;

macro_rules! err {
    () => {
        // todo
        // egui_web::console_error(format!("error at {}:{}", file!(), line!()));
    };
}

/// День
#[derive(Serialize, Deserialize, Clone, Copy, Ord, PartialOrd, Eq, PartialEq)]
pub struct Day(u64);

impl std::fmt::Debug for Day {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "Day({})", self.0)
    }
}

/// Итерация изучения слова, сколько ждать с последнего изучения, сколько раз повторить, показывать ли слово во время набора
#[derive(Serialize, Deserialize, Clone)]
struct LearnType {
    /// Сколько дней ждать с последнего изучения
    wait_days: u8,
    count: u8,
    show_word: bool,
}

impl std::fmt::Debug for LearnType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "(wait {}, count {}, {})",
            self.wait_days,
            self.count,
            if self.show_word { "show" } else { "not show" }
        )
    }
}

impl LearnType {
    fn show(wait_days: u8, count: u8) -> Self {
        LearnType {
            wait_days,
            count,
            show_word: true,
        }
    }

    fn guess(wait_days: u8, count: u8) -> Self {
        LearnType {
            wait_days,
            count,
            show_word: false,
        }
    }
}

impl LearnType {
    fn can_learn_today(&self, last_learn: Day, today: Day) -> bool {
        if today.0 >= last_learn.0 {
            today.0 - last_learn.0 >= self.wait_days as u64
        } else {
            false
        }
    }
}

/// Статистика написаний для слова, дня или вообще
#[derive(Default, Serialize, Deserialize, Clone, Copy, Debug)]
struct TypingStats {
    right: u64,
    wrong: u64,
}

/// Обозначает одну пару слов рус-англ или англ-рус в статистике
#[derive(Serialize, Deserialize, Clone, Debug)]
enum WordStatus {
    /// Мы знали это слово раньше, его изучать не надо
    KnowPreviously,

    /// Мусорное слово, артефакт от приблизительного парсинга текстового файла или субтитров
    TrashWord,

    /// Мы изучаем это слово
    ToLearn {
        translation: String,

        /// Когда это слово в последний раз изучали
        last_learn: Day,

        /// Количество learns, которое уже преодолено
        current_level: u8,

        /// Количество вводов для текущего уровня
        current_count: u8,

        /// Статистика
        stats: TypingStats,
    },

    // Мы знаем это слово
    Learned {
        translation: String,

        /// Статистика
        stats: TypingStats,
    },
}

impl WordStatus {
    fn register_attempt(
        &mut self,
        correct: bool,
        today: Day,
        day_stats: &mut DayStatistics,
        type_count: &[LearnType],
    ) {
        use WordStatus::*;
        match self {
            KnowPreviously | TrashWord | Learned { .. } => unreachable!(),
            ToLearn {
                stats,
                last_learn,
                translation,
                current_level,
                current_count,
            } => {
                if correct {
                    stats.right += 1;
                    day_stats.attempts.right += 1;
                } else {
                    stats.wrong += 1;
                    day_stats.attempts.wrong += 1;
                }

                if correct {
                    for learn in type_count.iter().skip(*current_level as _) {
                        if learn.can_learn_today(*last_learn, today) {
                            if *current_count + 1 != learn.count {
                                *current_count += 1;
                            } else {
                                *last_learn = today;
                                *current_level += 1;
                                *current_count = 0;
                            }
                            break;
                        }
                    }

                    if *current_level as usize == type_count.len() {
                        *self = WordStatus::Learned {
                            translation: translation.clone(),
                            stats: *stats,
                        };
                    }
                }
            }
        }
    }

    fn has_translation(&self, translation2: &str) -> bool {
        use WordStatus::*;
        match self {
            KnowPreviously | TrashWord => false,
            ToLearn { translation, .. } | Learned { translation, .. } => {
                translation == translation2
            }
        }
    }

    fn has_hint(&self, type_count: &[LearnType]) -> bool {
        use WordStatus::*;
        match self {
            KnowPreviously | TrashWord | Learned { .. } => false,
            ToLearn { current_level, .. } => type_count
                .get(*current_level as usize)
                .map(|x| x.show_word)
                .unwrap_or(false),
        }
    }

    fn can_learn_today(&self, today: Day, type_count: &[LearnType]) -> bool {
        if let WordStatus::ToLearn {
            last_learn,
            current_level,
            ..
        } = self
        {
            type_count
                .get(*current_level as usize)
                .map(|learn| learn.can_learn_today(*last_learn, today))
                .unwrap_or(false)
        } else {
            false
        }
    }

    fn translation(&self) -> Option<&str> {
        use WordStatus::*;
        if let ToLearn { translation, .. } | Learned { translation, .. } = self {
            Some(translation)
        } else {
            None
        }
    }

    fn translation_mut(&mut self) -> Option<&mut String> {
        use WordStatus::*;
        if let ToLearn { translation, .. } | Learned { translation, .. } = self {
            Some(translation)
        } else {
            None
        }
    }

    fn level(&self) -> Option<u8> {
        use WordStatus::*;
        if let ToLearn { current_level, .. } = self {
            Some(*current_level)
        } else {
            None
        }
    }

    fn overdue_days(&self, today: Day, type_count: &[LearnType]) -> u64 {
        use WordStatus::*;
        if let ToLearn {
            last_learn,
            current_level,
            ..
        } = self
        {
            let date_to_learn = last_learn.0 + type_count[*current_level as usize].wait_days as u64;
            if today.0 > date_to_learn {
                0
            } else {
                date_to_learn - today.0
            }
        } else {
            0
        }
    }

    fn attempts_remains(&self, today: Day, type_count: &[LearnType]) -> u8 {
        use WordStatus::*;
        if let ToLearn {
            last_learn,
            current_level,
            current_count,
            ..
        } = self
        {
            if let Some(learn) = type_count.get(*current_level as usize) {
                if learn.can_learn_today(*last_learn, today) {
                    learn.count - current_count
                } else {
                    0
                }
            } else {
                0
            }
        } else {
            0
        }
    }
}

/// Все слова в программе
#[derive(Default, Serialize, Deserialize, Clone, Debug)]
pub struct Words(BTreeMap<String, Vec<WordStatus>>);

enum WordsToAdd {
    KnowPreviously,
    TrashWord,
    ToLearn {
        learned: Vec<String>,
        translations: Vec<String>,
    },
}

struct WordsToLearn {
    known_words: Vec<String>,
    words_to_type: Vec<String>,
    words_to_guess: Vec<String>,
}

impl Words {
    fn calculate_known_words(&self) -> BTreeSet<String> {
        self.0.iter().map(|(word, _)| word.clone()).collect()
    }

    fn add_word(
        &mut self,
        word: String,
        info: WordsToAdd,
        today: Day,
        day_stats: &mut DayStatistics,
    ) {
        use WordsToAdd::*;
        let entry = self.0.entry(word.clone()).or_insert_with(Vec::new);
        match info {
            KnowPreviously => entry.push(WordStatus::KnowPreviously),
            TrashWord => entry.push(WordStatus::TrashWord),
            ToLearn {
                learned,
                translations,
            } => {
                for translation in &translations {
                    entry.push(WordStatus::ToLearn {
                        translation: translation.clone(),
                        last_learn: today,
                        current_level: 0,
                        current_count: 0,
                        stats: Default::default(),
                    });
                    day_stats.new_unknown_words_count += 1;
                }
                for translation in &learned {
                    entry.push(WordStatus::Learned {
                        translation: translation.clone(),
                        stats: Default::default(),
                    });
                    day_stats.new_unknown_words_count += 1;
                }
                for translation in translations {
                    self.0
                        .entry(translation)
                        .or_insert_with(Vec::new)
                        .push(WordStatus::ToLearn {
                            translation: word.clone(),
                            last_learn: today,
                            current_level: 0,
                            current_count: 0,
                            stats: Default::default(),
                        });
                }
                for translation in learned {
                    self.0
                        .entry(translation)
                        .or_insert_with(Vec::new)
                        .push(WordStatus::Learned {
                            translation: word.clone(),
                            stats: Default::default(),
                        });
                }
            }
        }
    }

    fn is_learned(&self, word: &str) -> bool {
        if let Some(word) = self.0.get(word) {
            for i in word {
                if matches!(i, WordStatus::ToLearn { .. }) {
                    return false;
                }
            }
            true
        } else {
            err!();
            true
        }
    }

    fn has_hint(&self, word: &str, type_count: &[LearnType]) -> bool {
        if let Some(word) = self.0.get(word) {
            word.iter().any(|x| x.has_hint(type_count))
        } else {
            false
        }
    }

    fn get_word_to_learn(&self, word: &str, today: Day, type_count: &[LearnType]) -> WordsToLearn {
        let mut known_words = Vec::new();
        let mut words_to_type = Vec::new();
        let mut words_to_guess = Vec::new();
        for i in self.0.get(word).unwrap() {
            if let WordStatus::ToLearn {
                translation,
                last_learn,
                current_level,
                ..
            } = i
            {
                for learn in type_count.iter().skip(*current_level as _) {
                    if learn.can_learn_today(*last_learn, today) {
                        if learn.show_word {
                            words_to_type.push(translation.clone());
                        } else {
                            words_to_guess.push(translation.clone());
                        }
                        break;
                    }
                }
                if type_count
                    .iter()
                    .skip(*current_level as _)
                    .all(|x| !x.can_learn_today(*last_learn, today))
                {
                    known_words.push(translation.clone());
                }
            } else if let WordStatus::Learned { translation, .. } = i {
                known_words.push(translation.clone());
            }
        }
        WordsToLearn {
            known_words,
            words_to_type,
            words_to_guess,
        }
    }

    fn get_words_to_learn_today(
        &self,
        today: Day,
        type_count: &[LearnType],
    ) -> (Vec<String>, Vec<String>) {
        let mut new = Vec::new();
        let mut repeat = Vec::new();
        for (word, statuses) in &self.0 {
            if statuses
                .iter()
                .any(|x| x.can_learn_today(today, type_count))
            {
                if statuses.iter().any(|x| x.level() == Some(0)) {
                    new.push(word.clone());
                } else {
                    repeat.push(word.clone());
                }
            }
        }
        (repeat, new)
    }

    fn register_attempt(
        &mut self,
        word: &str,
        translation: &str,
        correct: bool,
        today: Day,
        day_stats: &mut DayStatistics,
        type_count: &[LearnType],
    ) {
        if let Some(word) = self.0.get_mut(word) {
            for i in word {
                if i.has_translation(translation) {
                    i.register_attempt(correct, today, day_stats, type_count);
                    return;
                }
            }
            err!();
        } else {
            err!();
        }
    }

    fn calculate_word_statistics(&self) -> BTreeMap<WordType, u64> {
        let mut result = BTreeMap::new();
        for i in self.0.values().flatten() {
            use WordStatus::*;
            match i {
                KnowPreviously => *result.entry(WordType::Known).or_insert(0) += 1,
                TrashWord => *result.entry(WordType::Trash).or_insert(0) += 1,
                ToLearn { current_level, .. } => {
                    *result.entry(WordType::Level(*current_level)).or_insert(0) += 1
                }
                Learned { .. } => *result.entry(WordType::Learned).or_insert(0) += 1,
            }
        }
        result
    }

    fn calculate_attempts_statistics(&self) -> TypingStats {
        let mut result = TypingStats::default();
        for i in self.0.values().flatten() {
            if let WordStatus::ToLearn { stats, .. } = i {
                result.right += stats.right;
                result.wrong += stats.wrong;
            }
        }
        result
    }

    fn remove_word(&mut self, word: &str) {
        let translations: Vec<String> = self
            .0
            .remove(word)
            .unwrap()
            .into_iter()
            .filter_map(|x| x.translation().map(|x| x.to_owned()))
            .collect();

        for translation in translations {
            if let Some(to_edit) = self.0.get_mut(&translation) {
                *to_edit = to_edit
                    .iter()
                    .filter(|w| w.translation().map(|x| x != word).unwrap_or(true))
                    .cloned()
                    .collect();
            }
            if self.0.get(word).map(|x| x.is_empty()).unwrap_or(false) {
                self.0.remove(&translation);
            }
        }
    }

    fn rename_word(&mut self, word: &str, new_word: &str) {
        let status = self.0.remove(word).unwrap();
        let translations: Vec<String> = status
            .iter()
            .filter_map(|x| x.translation().map(|x| x.to_owned()))
            .collect();
        self.0.insert(new_word.to_owned(), status);

        for translation in translations {
            if let Some(to_edit) = self.0.get_mut(&translation) {
                *to_edit = to_edit
                    .iter()
                    .cloned()
                    .map(|mut w| {
                        if let Some(tr) = w.translation_mut() {
                            if tr == word {
                                *tr = new_word.to_string();
                            }
                        }
                        w
                    })
                    .collect();
            }
        }
    }

    fn max_overdue_days(&self, word: &str, today: Day, type_count: &[LearnType]) -> u64 {
        if let Some(trs) = self.0.get(word) {
            trs.iter()
                .map(|x| x.overdue_days(today, type_count))
                .max()
                .unwrap_or(0)
        } else {
            0
        }
    }

    fn max_attempts_remains(&self, word: &str, today: Day, type_count: &[LearnType]) -> u8 {
        if let Some(trs) = self.0.get(word) {
            trs.iter()
                .map(|x| x.attempts_remains(today, type_count))
                .max()
                .unwrap_or(0)
        } else {
            0
        }
    }

    fn can_learn_today(&self, word: &str, today: Day, type_count: &[LearnType]) -> bool {
        self.0
            .get(word)
            .map(|x| x.iter().any(|x| x.can_learn_today(today, type_count)))
            .unwrap_or(false)
    }
}

fn get_words_subtitles(subtitles: &str) -> Result<GetWordsResult, srtparse::ReaderError> {
    let subtitles = srtparse::from_str(subtitles)?;
    let text = subtitles
        .into_iter()
        .map(|x| x.text)
        .collect::<Vec<_>>()
        .join("\n");

    Ok(get_words(&text))
}

struct WordsWithContext(Vec<(String, Vec<std::ops::Range<usize>>)>);

struct GetWordsResult {
    text: String,
    words_with_context: WordsWithContext,
    words_count: usize,
    unique_words_count: usize,
}

fn is_word_symbol(c: char) -> bool {
    c.is_alphabetic() || c == '\'' || c == '-'
}

fn get_words(text: &str) -> GetWordsResult {
    let mut words_count = 0;
    let mut words = BTreeMap::new();
    let mut current_word: Option<(String, usize)> = None;
    for (i, c) in text
        .char_indices()
        .chain(std::iter::once((text.len(), '.')))
    {
        if is_word_symbol(c) {
            if let Some((word, _)) = &mut current_word {
                *word += &c.to_lowercase().collect::<String>();
            } else {
                current_word = Some((c.to_lowercase().collect(), i));
            }
        } else if let Some((word, start)) = &mut current_word {
            words_count += 1;
            words
                .entry(word.clone())
                .or_insert_with(Vec::new)
                .push(*start..i);
            current_word = None;
        }
    }
    let mut words: Vec<_> = words.into_iter().collect();

    words.sort_by_key(|x| std::cmp::Reverse(x.1.len()));

    let unique_words_count = words.len();

    GetWordsResult {
        text: text.to_owned(),
        words_with_context: WordsWithContext(words),
        words_count,
        unique_words_count,
    }
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct Settings {
    type_count: Vec<LearnType>,
    time_to_pause: f64,
    use_keyboard_layout: bool,
    keyboard_layout: KeyboardLayout,
    dpi: f32,
    #[serde(default)]
    white_theme: bool,
}

#[derive(Default, Serialize, Deserialize, Clone, Debug)]
struct KeyboardLayout {
    lang1: BTreeMap<char, char>,
    lang2: BTreeMap<char, char>,
}

impl KeyboardLayout {
    fn new(lang1: &str, lang2: &str) -> Result<KeyboardLayout, String> {
        let a: Vec<char> = lang1.chars().filter(|x| *x != '\n').collect();
        let b: Vec<char> = lang2.chars().filter(|x| *x != '\n').collect();
        if a.len() != b.len() {
            return Err(format!(
                "Lengths of symbols are not equal: {} ≠ {}",
                a.len(),
                b.len()
            ));
        }

        let mut error_reason = (' ', ' ');
        if a.iter().filter(|a| **a != ' ').any(|a| {
            b.iter().any(|x| {
                let result = *x == *a;
                if result {
                    error_reason = (*x, *a);
                }
                result
            })
        }) {
            return Err(format!("In first lang there is symbol '{}', which equals to symbol '{}' in the second lang.", error_reason.0, error_reason.1));
        }

        let mut result = Self {
            lang1: Default::default(),
            lang2: Default::default(),
        };

        for (a, b) in a.iter().zip(b.iter()) {
            result.lang1.insert(*a, *b);
            result.lang2.insert(*b, *a);
        }

        Ok(result)
    }

    fn change(&self, should_be: &str, to_change: &mut String) {
        let is_first_lang = self.lang2.contains_key(&should_be.chars().next().unwrap());
        let lang = if is_first_lang {
            &self.lang1
        } else {
            &self.lang2
        };
        *to_change = to_change
            .chars()
            .map(|x| {
                if let Some(c) = lang.get(&x).filter(|_| x != ' ') {
                    *c
                } else {
                    x
                }
            })
            .collect();
    }
}

impl Default for Settings {
    fn default() -> Self {
        Settings {
            type_count: vec![
                LearnType::show(0, 2),
                LearnType::guess(0, 3),
                LearnType::guess(2, 3),
                LearnType::guess(7, 2),
                LearnType::guess(20, 2),
            ],
            time_to_pause: 15.,
            use_keyboard_layout: false,
            keyboard_layout: Default::default(),
            dpi: 1.0,
            white_theme: false,
        }
    }
}

impl Settings {
    fn color_github_zero(&self) -> egui::Color32 {
        if self.white_theme {
            egui::Color32::from_gray(240)
        } else {
            egui::Color32::from_gray(24)
        }
    }

    fn color_github_high(&self) -> egui::Color32 {
        if self.white_theme {
            egui::Color32::from_rgb(33, 110, 57)
        } else {
            egui::Color32::from_rgba_unmultiplied(0, 255, 128, 255)
        }
    }

    fn color_github_low(&self) -> egui::Color32 {
        if self.white_theme {
            egui::Color32::from_rgb(155, 233, 168)
        } else {
            egui::Color32::from_rgba_unmultiplied(5, 101, 5, 255)
        }
    }

    fn color_github_month(&self) -> egui::Color32 {
        if self.white_theme {
            egui::Color32::from_rgba_unmultiplied(76, 76, 76, 255)
        } else {
            egui::Color32::WHITE
        }
    }

    fn color_github_year(&self) -> egui::Color32 {
        if self.white_theme {
            egui::Color32::from_rgba_unmultiplied(226, 31, 31, 255)
        } else {
            egui::Color32::RED
        }
    }

    fn color_delete(&self) -> egui::Color32 {
        if self.white_theme {
            egui::Color32::from_rgba_unmultiplied(213, 0, 0, 255)
        } else {
            egui::Color32::RED
        }
    }

    fn color_add(&self) -> egui::Color32 {
        if self.white_theme {
            egui::Color32::from_rgba_unmultiplied(0, 171, 0, 255)
        } else {
            egui::Color32::RED
        }
    }

    fn color_error(&self) -> egui::Color32 {
        if self.white_theme {
            egui::Color32::from_rgba_unmultiplied(255, 0, 0, 255)
        } else {
            egui::Color32::RED
        }
    }

    fn color_red_field_1(&self) -> egui::Color32 {
        if self.white_theme {
            egui::Color32::from_rgba_unmultiplied(255, 0, 0, 255)
        } else {
            egui::Color32::RED
        }
    }

    fn color_red_field_2(&self) -> egui::Color32 {
        if self.white_theme {
            egui::Color32::from_rgba_unmultiplied(224, 0, 0, 200)
        } else {
            egui::Color32::from_rgb_additive(128, 0, 0)
        }
    }

    fn color_red_field_3(&self) -> egui::Color32 {
        if self.white_theme {
            egui::Color32::from_rgba_unmultiplied(255, 128, 128, 255)
        } else {
            egui::Color32::from_rgb_additive(255, 128, 128)
        }
    }

    fn color_green_field_1(&self) -> egui::Color32 {
        if self.white_theme {
            egui::Color32::from_rgba_unmultiplied(0, 255, 0, 255)
        } else {
            egui::Color32::GREEN
        }
    }

    fn color_green_field_2(&self) -> egui::Color32 {
        if self.white_theme {
            egui::Color32::from_rgba_unmultiplied(0, 195, 63, 201)
        } else {
            egui::Color32::from_rgb_additive(0, 128, 0)
        }
    }

    fn color_green_field_3(&self) -> egui::Color32 {
        if self.white_theme {
            egui::Color32::from_rgba_unmultiplied(47, 198, 0, 191)
        } else {
            egui::Color32::from_rgb_additive(128, 255, 128)
        }
    }
}

#[derive(Serialize, Deserialize, Clone, Debug, Ord, PartialOrd, Eq, PartialEq)]
pub enum WordType {
    Known,
    Trash,
    Level(u8),
    Learned,
}

#[derive(Default, Serialize, Deserialize, Clone, Debug)]
pub struct DayStatistics {
    attempts: TypingStats,
    new_unknown_words_count: u64,
    word_count_by_level: BTreeMap<WordType, u64>,
    working_time: f64,
}

#[derive(Default, Serialize, Deserialize, Clone, Debug)]
pub struct Statistics {
    by_day: BTreeMap<Day, DayStatistics>,
}

mod gui {
    use super::*;
    use egui::*;

    struct ClosableWindow<T: WindowTrait>(Option<T>);

    impl<T: WindowTrait> Default for ClosableWindow<T> {
        fn default() -> Self {
            Self(None)
        }
    }

    trait WindowTrait {
        fn create_window(&self) -> Window<'static>;
    }

    impl<T: WindowTrait> ClosableWindow<T> {
        fn new(t: T) -> Self {
            Self(Some(t))
        }

        /// Возвращение true в f означает что самого себя надо закрыть. Возвращение true в ui означает что окно закрылось
        fn ui(&mut self, ctx: &CtxRef, f: impl FnOnce(&mut T, &mut Ui) -> bool) -> bool {
            if let Some(t) = &mut self.0 {
                let mut opened = true;
                let mut want_to_be_closed = false;

                t.create_window()
                    .open(&mut opened)
                    .show(ctx, |ui| want_to_be_closed = f(t, ui));

                if !opened || want_to_be_closed {
                    self.0 = None;
                    return true;
                }
            }
            false
        }
    }

    pub struct Program {
        words: Words,
        settings: Settings,
        stats: Statistics,

        /// Известные, мусорные, выученные, добавленные слова, необходимо для фильтрации после добавления слова
        known_words: BTreeSet<String>,
        learn_window: LearnWordsWindow,
        load_text_window: ClosableWindow<LoadTextWindow>,
        add_words_window: ClosableWindow<AddWordsWindow>,
        add_custom_words_window: ClosableWindow<AddCustomWordsWindow>,

        full_stats_window: ClosableWindow<FullStatsWindow>,
        percentage_graph_window: ClosableWindow<PercentageGraphWindow>,
        github_activity_window: ClosableWindow<GithubActivityWindow>,

        import_window: ClosableWindow<ImportWindow>,
        export_window: ClosableWindow<ExportWindow>,
        settings_window: ClosableWindow<SettingsWindow>,
        about_window: ClosableWindow<AboutWindow>,
        search_words_window: ClosableWindow<SearchWordsWindow>,
        edit_word_window: ClosableWindow<EditWordWindow>,
        info_window: ClosableWindow<InfoWindow>,
        synchronous_subtitles_window: ClosableWindow<SynchronousSubtitlesWindow>,
    }

    impl Program {
        pub fn new(
            words: Words,
            settings: Settings,
            stats: Statistics,
            today: Day,
            working_time: f64,
            rng: &mut Rand,
        ) -> Self {
            let learn_window = LearnWordsWindow::new(&words, today, &settings.type_count, rng);
            let known_words = words.calculate_known_words();

            let mut result = Self {
                words,
                settings,
                stats,

                known_words,
                learn_window,
                load_text_window: Default::default(),
                add_words_window: Default::default(),
                add_custom_words_window: Default::default(),

                full_stats_window: Default::default(),
                percentage_graph_window: Default::default(),
                github_activity_window: Default::default(),

                import_window: Default::default(),
                export_window: Default::default(),
                settings_window: Default::default(),
                about_window: Default::default(),
                search_words_window: Default::default(),
                edit_word_window: Default::default(),
                info_window: Default::default(),
                synchronous_subtitles_window: Default::default(),
            };

            result.open_activity(today, working_time);

            result
        }

        pub fn get_settings(&self) -> &Settings {
            &self.settings
        }

        pub fn save_to_string(&mut self, today: Day, working_time: f64) -> String {
            self.update_day_statistics(today, working_time);
            ron::to_string(&(&self.words, &self.settings, &self.stats)).unwrap()
        }

        pub fn save(&mut self, today: Day, working_time: f64) {
            quad_storage::STORAGE.lock().unwrap().set(
                "learn_words_data",
                &self.save_to_string(today, working_time),
            );
        }

        pub fn load() -> (Words, Settings, Statistics) {
            quad_storage::STORAGE
                .lock()
                .unwrap()
                .get("learn_words_data")
                .map(|x| Self::load_from_string(&x).unwrap())
                .unwrap_or_default()
        }

        pub fn load_from_string(s: &str) -> Result<(Words, Settings, Statistics), ron::Error> {
            ron::from_str::<(Words, Settings, Statistics)>(s)
        }

        pub fn update_day_statistics(&mut self, today: Day, working_time: f64) {
            let today = &mut self.stats.by_day.entry(today).or_default();
            today.working_time = working_time;
            today.word_count_by_level = self.words.calculate_word_statistics();
        }

        pub fn open_activity(&mut self, today: Day, working_time: f64) {
            self.update_day_statistics(today, working_time);
            self.github_activity_window =
                ClosableWindow::new(GithubActivityWindow::new(&self.stats, today));
        }

        pub fn ui(
            &mut self,
            ctx: &CtxRef,
            today: Day,
            working_time: &mut f64,
            rng: &mut Rand,
            paused: bool,
        ) {
            TopBottomPanel::top("top").show(ctx, |ui| {
                menu::bar(ui, |ui| {
                    menu::menu(ui, "Data", |ui| {
                        if ui.button("Export").clicked() {
                            self.export_window = ClosableWindow::new(ExportWindow::new(
                                self.save_to_string(today, *working_time),
                            ));
                        }
                        if ui.button("Import").clicked() {
                            self.import_window = ClosableWindow::new(ImportWindow::new());
                        }
                    });
                    menu::menu(ui, "Add words", |ui| {
                        if ui.button("From text").clicked() {
                            self.load_text_window = ClosableWindow::new(LoadTextWindow::new(false));
                        }
                        if ui.button("From subtitles").clicked() {
                            self.load_text_window = ClosableWindow::new(LoadTextWindow::new(true));
                        }
                        if ui.button("Manually").clicked() {
                            self.add_custom_words_window = ClosableWindow::new(Default::default());
                        }
                        ui.separator();
                        if ui.button("Synchronous subtitles").clicked() {
                            self.synchronous_subtitles_window =
                                ClosableWindow::new(SynchronousSubtitlesWindow::new());
                        }
                    });
                    if ui.button("Search").clicked() {
                        self.search_words_window =
                            ClosableWindow::new(SearchWordsWindow::new(String::new(), &self.words));
                    }
                    menu::menu(ui, "Statistics", |ui| {
                        if ui.button("Full").clicked() {
                            self.full_stats_window = ClosableWindow::new(FullStatsWindow {
                                time: self
                                    .stats
                                    .by_day
                                    .values()
                                    .map(|x| x.working_time)
                                    .sum::<f64>(),
                                attempts: self.words.calculate_attempts_statistics(),
                                word_count_by_level: self.words.calculate_word_statistics(),
                            });
                        }
                        if ui.button("GitHub-like").clicked() {
                            self.open_activity(today, *working_time);
                        }
                        ui.separator();
                        if ui.button("Attempts by day").clicked() {
                            self.update_day_statistics(today, *working_time);
                            self.percentage_graph_window =
                                ClosableWindow::new(PercentageGraphWindow {
                                    name: "Attempts by day",
                                    values: self
                                        .stats
                                        .by_day
                                        .iter()
                                        .map(|(k, v)| {
                                            (
                                                *k,
                                                vec![
                                                    v.attempts.right as f64,
                                                    v.attempts.wrong as f64,
                                                ],
                                            )
                                        })
                                        .collect(),
                                    names: vec![
                                        "Right attempts".to_string(),
                                        "Wrong attempts".to_string(),
                                    ],
                                    stackplot: false,
                                    moving: false,
                                });
                        }
                        if ui.button("Time by day").clicked() {
                            self.update_day_statistics(today, *working_time);
                            self.percentage_graph_window =
                                ClosableWindow::new(PercentageGraphWindow {
                                    name: "Time by day",
                                    values: self
                                        .stats
                                        .by_day
                                        .iter()
                                        .map(|(k, v)| (*k, vec![v.working_time]))
                                        .collect(),
                                    names: vec!["Working time".to_string()],
                                    stackplot: false,
                                    moving: false,
                                });
                        }
                        if ui.button("Words by day").clicked() {
                            self.update_day_statistics(today, *working_time);
                            let available_types: BTreeSet<WordType> = self
                                .stats
                                .by_day
                                .values()
                                .map(|x| x.word_count_by_level.keys().cloned())
                                .flatten()
                                .collect();
                            use WordType::*;
                            self.percentage_graph_window =
                                ClosableWindow::new(PercentageGraphWindow {
                                    name: "Words by day",
                                    values: self
                                        .stats
                                        .by_day
                                        .iter()
                                        .map(|(k, v)| {
                                            (
                                                *k,
                                                available_types
                                                    .iter()
                                                    .map(|x| {
                                                        v.word_count_by_level
                                                            .get(x)
                                                            .copied()
                                                            .unwrap_or(0)
                                                            as f64
                                                    })
                                                    .collect(),
                                            )
                                        })
                                        .collect(),
                                    names: available_types
                                        .iter()
                                        .map(|x| match x {
                                            Known => "Known".to_string(),
                                            Trash => "Trash".to_string(),
                                            Level(l) => format!("Level {}", l),
                                            Learned => "Learned".to_string(),
                                        })
                                        .collect(),
                                    stackplot: false,
                                    moving: false,
                                });
                        }
                    });
                    if ui.button("Settings").clicked() {
                        self.settings_window =
                            ClosableWindow::new(SettingsWindow::new(&self.settings));
                    }
                    if ui.button("About").clicked() {
                        self.about_window = ClosableWindow::new(AboutWindow);
                    }
                });
            });

            let mut save = false;
            self.learn_window.ui(
                ctx,
                &mut self.words,
                today,
                &mut self.stats.by_day.entry(today).or_default(),
                &self.settings,
                &mut save,
                rng,
            );
            if save {
                self.save(today, *working_time);
            }

            let window = &mut self.load_text_window;
            let words = &self.words;
            let add_words_window = &mut self.add_words_window;
            let info_window = &mut self.info_window;
            let settings = &self.settings;
            window.ui(ctx, |t, ui| {
                if let Some((words, stats)) = t.ui(ui, words, settings) {
                    if !words.words_with_context.0.is_empty() {
                        *add_words_window = ClosableWindow::new(AddWordsWindow::new(
                            words.text,
                            words.words_with_context,
                        ));
                    }
                    *info_window = ClosableWindow::new(InfoWindow(vec![
                        "Info about words count in this text.".to_string(),
                        format!("Total: {}", words.words_count),
                        format!("Unique: {}", words.unique_words_count),
                        format!(
                            "Filtered: {}",
                            stats.filtered_known + stats.filtered_learned
                        ),
                        format!("   Known: {}", stats.filtered_known),
                        format!("   Learning: {}", stats.filtered_learned),
                        format!("Unknown: {}", stats.unknown_words),
                    ]));
                    true
                } else {
                    false
                }
            });

            let closed = self.import_window.ui(ctx, |t, ui| {
                if let Some((words1, settings1, stats1)) = t.ui(ui, &self.settings) {
                    self.words = words1;
                    self.settings = settings1;
                    self.stats = stats1;
                    ui.ctx().set_pixels_per_point(self.settings.dpi);
                    if let Some(time) = self.stats.by_day.get(&today).map(|x| x.working_time) {
                        *working_time = time;
                    }
                    true
                } else {
                    false
                }
            });
            if closed {
                self.learn_window
                    .update(&self.words, today, &self.settings.type_count, rng);
            }

            let mut save = false;
            self.settings_window.ui(ctx, |t, ui| {
                t.ui(ui, &mut self.settings, &mut save);
                false
            });
            if save {
                self.save(today, *working_time);
            }

            let mut save = false;
            let closed = self.add_words_window.ui(ctx, |t, ui| {
                if let Some((word, to_add, close)) = t.ui(
                    ui,
                    &mut self.search_words_window,
                    &mut self.synchronous_subtitles_window,
                    &self.words,
                ) {
                    self.words.add_word(
                        word,
                        to_add,
                        today,
                        self.stats.by_day.entry(today).or_default(),
                    );
                    save = true;
                    close
                } else {
                    false
                }
            });
            if closed {
                self.learn_window
                    .update(&self.words, today, &self.settings.type_count, rng);
                self.known_words = self.words.calculate_known_words();
                self.save(today, *working_time);
            }
            if save {
                self.save(today, *working_time);
            }

            let mut save = false;
            let closed = self.add_custom_words_window.ui(ctx, |t, ui| {
                if let Some((word, to_add)) = t.ui(ui) {
                    self.words.add_word(
                        word,
                        to_add,
                        today,
                        self.stats.by_day.entry(today).or_default(),
                    );
                    save = true;
                }
                false
            });
            if closed {
                self.learn_window
                    .update(&self.words, today, &self.settings.type_count, rng);
                self.known_words = self.words.calculate_known_words();
                self.save(today, *working_time);
            }
            if save {
                self.save(today, *working_time);
            }

            self.full_stats_window.ui(ctx, |t, ui| {
                t.ui(ui);
                false
            });

            self.percentage_graph_window.ui(ctx, |t, ui| {
                t.ui(ui);
                false
            });

            self.github_activity_window.ui(ctx, |t, ui| {
                t.ui(ui, &self.settings);
                false
            });

            self.about_window.ui(ctx, |t, ui| {
                t.ui(ui);
                false
            });

            self.info_window.ui(ctx, |t, ui| {
                t.ui(ui);
                false
            });

            self.export_window.ui(ctx, |t, ui| {
                t.ui(ui);
                false
            });

            self.synchronous_subtitles_window.ui(ctx, |t, ui| {
                t.ui(ui, &self.settings);
                false
            });

            let mut edit_word = None;
            self.search_words_window.ui(ctx, |t, ui| {
                edit_word = t.ui(ui, &self.words);
                false
            });
            if let Some(edit_word) = edit_word {
                self.edit_word_window = ClosableWindow::new(EditWordWindow::new(edit_word));
            }

            let mut update_search = false;
            let mut save = false;
            let closed = self.edit_word_window.ui(ctx, |t, ui| {
                let result = t.ui(ui, &mut self.words, &mut save, &self.settings);
                update_search = result.1;
                result.0
            });
            if update_search {
                if let Some(window) = &mut self.search_words_window.0 {
                    window.update(&self.words);
                }
            }
            if closed || update_search {
                self.known_words = self.words.calculate_known_words();
                self.save(today, *working_time);
            }
            if save {
                self.save(today, *working_time);
            }

            egui::TopBottomPanel::bottom("bottom").show(ctx, |ui| {
                let today = &self.stats.by_day.entry(today).or_default();
                ui.monospace(format!(
                    "Working time: {:6} | Attempts: {:4} | New words: {:4}{}",
                    print_time(*working_time),
                    today.attempts.right + today.attempts.wrong,
                    today.new_unknown_words_count,
                    if paused { "| PAUSED" } else { "" }
                ));
            });
        }
    }

    fn print_time(time: f64) -> String {
        if time > 3600. {
            format!(
                "{}:{:02}:{:02}",
                time as u32 / 3600,
                time as u32 % 3600 / 60,
                time as u32 % 60
            )
        } else if time > 60. {
            format!("{:02}:{:02}", time as u32 / 60, time as u32 % 60)
        } else {
            format!("{:02}", time as u32)
        }
    }

    struct LoadTextWindow {
        load_subtitles: bool,
        subtitles_error: Option<String>,
        text: String,
    }

    impl WindowTrait for LoadTextWindow {
        fn create_window(&self) -> Window<'static> {
            Window::new(if self.load_subtitles {
                "Words from subs"
            } else {
                "Words from text"
            })
            .vscroll(true)
            .fixed_size((200., 200.))
            .collapsible(false)
        }
    }

    #[derive(Default)]
    struct LoadTextStats {
        filtered_known: usize,
        filtered_learned: usize,
        unknown_words: usize,
    }

    impl LoadTextWindow {
        fn new(load_subtitles: bool) -> Self {
            Self {
                load_subtitles,
                subtitles_error: None,
                text: String::new(),
            }
        }

        fn ui(
            &mut self,
            ui: &mut Ui,
            data: &Words,
            settings: &Settings,
        ) -> Option<(GetWordsResult, LoadTextStats)> {
            let mut action = None;
            ui.horizontal(|ui| {
                if ui.button("Use this text").clicked() {
                    let text = &self.text;

                    let words = if self.load_subtitles {
                        match get_words_subtitles(text) {
                            Ok(words) => Some(words),
                            Err(error) => {
                                self.subtitles_error = Some(format!("{:#?}", error));
                                None
                            }
                        }
                    } else {
                        Some(get_words(text))
                    };
                    if let Some(mut words) = words {
                        let mut stats = LoadTextStats::default();
                        words
                            .words_with_context
                            .0
                            .retain(|x| match data.0.get(&x.0) {
                                Some(x)
                                    if x.iter()
                                        .any(|x| matches!(x, WordStatus::ToLearn { .. })) =>
                                {
                                    stats.filtered_learned += 1;
                                    false
                                }
                                Some(_) => {
                                    stats.filtered_known += 1;
                                    false
                                }
                                None => {
                                    stats.unknown_words += 1;
                                    true
                                }
                            });
                        action = Some((words, stats));
                    }
                }
            });
            if let Some(error) = &self.subtitles_error {
                ui.separator();
                ui.horizontal_wrapped(|ui| {
                    ui.spacing_mut().item_spacing.x = 0.;
                    ui.add(
                        Label::new("Error: ")
                            .text_color(settings.color_error())
                            .monospace(),
                    );
                    ui.monospace(error);
                });
            }
            ui.separator();
            ui.text_edit_multiline(&mut self.text);
            action
        }
    }

    struct ExportWindow {
        text: String,
    }

    impl WindowTrait for ExportWindow {
        fn create_window(&self) -> Window<'static> {
            Window::new("Export data")
                .vscroll(true)
                .fixed_size((200., 200.))
                .collapsible(false)
        }
    }

    impl ExportWindow {
        fn new(text: String) -> Self {
            Self { text }
        }

        fn ui(&mut self, ui: &mut Ui) {
            ui.label("Copy from this field: Ctrl+A, Ctrl+C.");

            #[cfg(target_arch = "wasm32")]
            if ui.button("Download as file").clicked() {
                download_as_file(&self.text);
            }

            ui.text_edit_multiline(&mut self.text);
        }
    }

    struct ImportWindow {
        text: String,
        error: Option<String>,
    }

    impl WindowTrait for ImportWindow {
        fn create_window(&self) -> Window<'static> {
            Window::new("Import data")
                .vscroll(true)
                .fixed_size((200., 200.))
                .collapsible(false)
        }
    }

    impl ImportWindow {
        fn new() -> Self {
            Self {
                text: String::new(),
                error: None,
            }
        }

        fn ui(
            &mut self,
            ui: &mut Ui,
            settings: &Settings,
        ) -> Option<(Words, Settings, Statistics)> {
            let mut action = None;
            ui.horizontal(|ui| {
                if ui.button("Use this text").clicked() {
                    match Program::load_from_string(&self.text) {
                        Ok(result) => action = Some(result),
                        Err(error) => {
                            self.error = Some(format!("{:#?}", error));
                        }
                    }
                }
            });
            if let Some(error) = &self.error {
                ui.separator();
                ui.horizontal_wrapped(|ui| {
                    ui.spacing_mut().item_spacing.x = 0.;
                    ui.add(
                        Label::new("Error: ")
                            .text_color(settings.color_error())
                            .monospace(),
                    );
                    ui.monospace(error);
                });
            }
            ui.separator();
            ui.text_edit_multiline(&mut self.text);
            action
        }
    }

    struct SettingsWindow {
        lang1: String,
        lang2: String,
        want_to_use_keyboard_layout: bool,
        info: Option<Result<String, String>>,
    }

    impl WindowTrait for SettingsWindow {
        fn create_window(&self) -> Window<'static> {
            Window::new("Settings")
                .vscroll(false)
                .fixed_size((300., 100.))
                .collapsible(false)
        }
    }

    impl SettingsWindow {
        fn new(settings: &Settings) -> Self {
            let mut result = Self {
                lang1: String::new(),
                lang2: String::new(),
                want_to_use_keyboard_layout: false,
                info: None,
            };
            if settings.use_keyboard_layout {
                result.lang1 = settings.keyboard_layout.lang1.keys().copied().collect();
                result.lang2 = settings.keyboard_layout.lang1.values().copied().collect();
            }
            result
        }

        fn ui(&mut self, ui: &mut Ui, settings: &mut Settings, save: &mut bool) {
            ui.horizontal(|ui| {
                ui.label("Theme: ");
                if !settings.white_theme {
                    if ui
                        .add(Button::new("☀").frame(false))
                        .on_hover_text("Switch to light mode")
                        .clicked()
                    {
                        settings.white_theme = !settings.white_theme;
                        ui.ctx().set_visuals(Visuals::light());
                        *save = true;
                    }
                } else {
                    if ui
                        .add(Button::new("🌙").frame(false))
                        .on_hover_text("Switch to dark mode")
                        .clicked()
                    {
                        settings.white_theme = !settings.white_theme;
                        ui.ctx().set_visuals(Visuals::dark());
                        *save = true;
                    }
                }
            });

            ui.separator();

            ui.horizontal(|ui| {
                ui.label("Inaction time for pause: ");
                if ui
                    .add(
                        egui::DragValue::new(&mut settings.time_to_pause)
                            .speed(0.1)
                            .clamp_range(0.0..=100.0)
                            .min_decimals(0)
                            .max_decimals(2),
                    )
                    .changed()
                {
                    *save = true;
                }
            });

            ui.separator();

            ui.horizontal(|ui| {
                let scale_factor = 1.05;
                ui.label(format!("Scale: {:.2}", settings.dpi));
                if ui
                    .add(egui::widgets::Button::new(" + ").text_style(egui::TextStyle::Monospace))
                    .clicked()
                {
                    settings.dpi *= scale_factor;
                    *save = true;
                }
                if ui
                    .add(egui::widgets::Button::new(" - ").text_style(egui::TextStyle::Monospace))
                    .clicked()
                {
                    settings.dpi /= scale_factor;
                    *save = true;
                }
                ui.ctx().set_pixels_per_point(settings.dpi);
            });

            ui.separator();

            ui.collapsing("Automatic change of keyboard layout", |ui| {
                if !self.want_to_use_keyboard_layout && settings.use_keyboard_layout {
                    self.want_to_use_keyboard_layout = true;
                    *save = true;
                }
                ui.checkbox(
                    &mut self.want_to_use_keyboard_layout,
                    "Use automatic change of keyboard layout",
                );
                if self.want_to_use_keyboard_layout {
                    ui.separator();
                    ui.label("Type all letters on your keyboard in first field, and then in the same order symbols in the second field. Newline is ignored. If you can't type some symbol, you can use space. Count of symbols except newline must be the same of both fields.");
                    ui.label("First language:");
                    ui.text_edit_multiline(&mut self.lang1);
                    ui.label("Second language:");
                    ui.text_edit_multiline(&mut self.lang2);
                    if ui.button("Use this keyboard layout").clicked() {
                        match KeyboardLayout::new(&self.lang1, &self.lang2) {
                            Ok(ok) => {
                                settings.use_keyboard_layout = true;
                                settings.keyboard_layout = ok;
                                self.info = Some(Ok("Used!".to_string()));
                                *save = true;
                            }
                            Err(err) => {
                                self.info = Some(Err(err));
                            }
                        }
                    }
                    if let Some(info) = &self.info {
                        match info {
                            Ok(ok) => {
                                ui.label(ok);
                            }
                            Err(err) => {
                                ui.horizontal_wrapped(|ui| {
                                    ui.spacing_mut().item_spacing.x = 0.;
                                    ui.add(Label::new("Error: ").text_color(settings.color_error()).monospace());
                                    ui.monospace(err);
                                });
                            }
                        }
                    }
                } else {
                    settings.use_keyboard_layout = false;
                }
            });

            ui.separator();

            ui.collapsing("Repeats", |ui| {
                let mut delete = None;
                let color_delete = settings.color_delete();
                let color_add = settings.color_add();
                for (pos, i) in settings.type_count.iter_mut().enumerate() {
                    ui.horizontal(|ui| {
                        ui.label(format!("{}.", pos));
                        ui.separator();
                        ui.label("Wait days: ");
                        if ui
                            .add(
                                egui::DragValue::new(&mut i.wait_days)
                                    .speed(0.1)
                                    .clamp_range(0.0..=99.0)
                                    .min_decimals(0)
                                    .max_decimals(0),
                            )
                            .changed()
                        {
                            *save = true;
                        }
                        ui.separator();
                        ui.label("Count: ");
                        if ui
                            .add(
                                egui::DragValue::new(&mut i.count)
                                    .speed(0.1)
                                    .clamp_range(0.0..=99.0)
                                    .min_decimals(0)
                                    .max_decimals(0),
                            )
                            .changed()
                        {
                            *save = true;
                        }
                        ui.separator();
                        ui.checkbox(&mut i.show_word, "Show hint");
                        ui.separator();
                        if ui
                            .add(Button::new("Delete").text_color(color_delete))
                            .clicked()
                        {
                            delete = Some(pos);
                        }
                    });
                }
                ui.separator();
                if ui.add(Button::new("Add").text_color(color_add)).clicked() {
                    settings.type_count.push(LearnType {
                        wait_days: 0,
                        count: 1,
                        show_word: false,
                    });
                    *save = true;
                }
                if let Some(pos) = delete {
                    settings.type_count.remove(pos);
                    *save = true;
                }
            });
        }
    }

    struct InfoWindow(Vec<String>);

    impl WindowTrait for InfoWindow {
        fn create_window(&self) -> Window<'static> {
            Window::new("Info")
                .vscroll(false)
                .fixed_size((200., 50.))
                .collapsible(false)
        }
    }

    impl InfoWindow {
        fn ui(&mut self, ui: &mut Ui) {
            for i in &self.0 {
                ui.label(i);
            }
        }
    }

    struct AboutWindow;

    impl WindowTrait for AboutWindow {
        fn create_window(&self) -> Window<'static> {
            Window::new("About")
                .vscroll(false)
                .fixed_size((320., 100.))
                .collapsible(false)
        }
    }

    impl AboutWindow {
        fn ui(&mut self, ui: &mut Ui) {
            ui.heading("Learn Words");
            ui.separator();
            ui.label("This is the program to learn words in foreign languages.");
            ui.separator();
            ui.horizontal_wrapped(|ui| {
                ui.spacing_mut().item_spacing.x = 0.;
                ui.add(egui::Label::new("Version: ").strong());
                ui.label(env!("CARGO_PKG_VERSION"));
            });
            ui.horizontal_wrapped(|ui| {
                ui.spacing_mut().item_spacing.x = 0.;
                ui.add(egui::Label::new("Author: ").strong());
                ui.label("Ilya Sheprut");
            });
            ui.horizontal_wrapped(|ui| {
                ui.spacing_mut().item_spacing.x = 0.;
                ui.add(egui::Label::new("License: ").strong());
                ui.label("MIT or Apache 2.0");
            });
            ui.horizontal_wrapped(|ui| {
                ui.spacing_mut().item_spacing.x = 0.;
                ui.add(egui::Label::new("Repository: ").strong());
                ui.hyperlink("https://github.com/optozorax/learn_words");
            });
        }
    }

    struct SearchWordsWindow {
        search_string: String,
        found_variants: Vec<String>,
        show_inners: bool,
    }

    impl WindowTrait for SearchWordsWindow {
        fn create_window(&self) -> Window<'static> {
            Window::new("Search words")
                .vscroll(false)
                .fixed_size((200., 300.))
                .collapsible(false)
        }
    }

    impl SearchWordsWindow {
        fn new(search_string: String, words: &Words) -> Self {
            let mut result = Self {
                search_string,
                found_variants: Vec::new(),
                show_inners: false,
            };
            result.update(words);
            result
        }

        fn update_new(&mut self, search_string: String, words: &Words) {
            if search_string != self.search_string {
                self.search_string = search_string;
                self.update(words);
            }
        }

        fn update(&mut self, words: &Words) {
            const ACCEPTED_LEVENSHTEIN: usize = 4;
            let mut results = Vec::new();
            for word in words.0.keys() {
                let levenshtein = strsim::levenshtein(word, &self.search_string);
                if levenshtein < ACCEPTED_LEVENSHTEIN {
                    let jaro = strsim::jaro(word, &self.search_string);
                    results.push((levenshtein, jaro, word.clone()));
                }
            }
            results.sort_by(|a, b| {
                if a.0 == b.0 {
                    a.1.partial_cmp(&b.1).unwrap()
                } else {
                    a.0.cmp(&b.0)
                }
            });
            self.found_variants = results.into_iter().map(|(_, _, w)| w).collect();
        }

        fn find_word(this: &mut Option<Self>, search_string: String, words: &Words) {
            if let Some(window) = this {
                window.update_new(search_string, words);
            } else {
                *this = Some(Self::new(search_string, words));
            }
        }

        fn ui(&mut self, ui: &mut Ui, words: &Words) -> Option<String> {
            if ui
                .add(
                    TextEdit::singleline(&mut self.search_string)
                        .hint_text("Type here to find word..."),
                )
                .changed()
            {
                self.update(words);
            }
            ui.checkbox(&mut self.show_inners, "Show inners");
            ui.separator();
            let mut edit_word = None;
            ScrollArea::vertical().max_height(200.0).show(ui, |ui| {
                if self.search_string.is_empty() {
                    if self.show_inners {
                        for (n, (word, translations)) in words.0.iter().enumerate() {
                            ui.with_layout(Layout::right_to_left(), |ui| {
                                if ui.button("✏").on_hover_text("Edit").clicked() {
                                    edit_word = Some(word.clone());
                                }
                                ui.with_layout(Layout::left_to_right(), |ui| {
                                    ui.heading(format!("{}. {}", n, word));
                                });
                            });
                            for word_status in translations {
                                ui.allocate_space(egui::vec2(1.0, 5.0));
                                word_status_show_ui(word_status, ui);
                            }
                            ui.separator();
                        }
                    } else {
                        for (n, word) in words.0.keys().enumerate() {
                            ui.with_layout(Layout::right_to_left(), |ui| {
                                if ui.button("✏").on_hover_text("Edit").clicked() {
                                    edit_word = Some(word.clone());
                                }
                                ui.with_layout(Layout::left_to_right(), |ui| {
                                    ui.label(format!("{}. {}", n, word));
                                });
                            });
                        }
                    }
                } else if self.show_inners {
                    for (word, translations) in self
                        .found_variants
                        .iter()
                        .map(|x| (x, words.0.get(x).unwrap()))
                    {
                        ui.with_layout(Layout::right_to_left(), |ui| {
                            if ui.button("✏").on_hover_text("Edit").clicked() {
                                edit_word = Some(word.clone());
                            }
                            ui.with_layout(Layout::left_to_right(), |ui| {
                                ui.heading(word);
                            });
                        });
                        for word_status in translations {
                            ui.allocate_space(egui::vec2(1.0, 5.0));
                            word_status_show_ui(word_status, ui);
                        }
                        ui.separator();
                    }
                } else {
                    for word in &self.found_variants {
                        ui.with_layout(Layout::right_to_left(), |ui| {
                            if ui.button("✏").on_hover_text("Edit").clicked() {
                                edit_word = Some(word.clone());
                            }
                            ui.with_layout(Layout::left_to_right(), |ui| {
                                ui.label(word);
                            });
                        });
                    }
                }
            });
            edit_word
        }
    }

    struct EditWordWindow {
        word: String,
        word_to_edit: String,
    }

    impl WindowTrait for EditWordWindow {
        fn create_window(&self) -> Window<'static> {
            Window::new("Edit word")
                .vscroll(true)
                .fixed_size((200., 300.))
                .collapsible(false)
        }
    }

    impl EditWordWindow {
        fn new(word: String) -> Self {
            Self {
                word: word.clone(),
                word_to_edit: word,
            }
        }

        fn ui(
            &mut self,
            ui: &mut Ui,
            words: &mut Words,
            save: &mut bool,
            settings: &Settings,
        ) -> (bool, bool) {
            ui.label("Please not edit words while typing in learning words window!");
            if let Some(getted) = words.0.get_mut(&self.word) {
                let mut remove_word = false;
                ui.with_layout(Layout::right_to_left(), |ui| {
                    if ui
                        .add(Button::new("Delete").text_color(settings.color_delete()))
                        .clicked()
                    {
                        remove_word = true;
                        *save = true;
                    }
                    ui.with_layout(Layout::left_to_right(), |ui| {
                        if ui.text_edit_singleline(&mut self.word_to_edit).changed() {
                            *save = true;
                        }
                    });
                });
                let mut rename = None;
                let mut delete = None;
                for (pos, word) in getted.iter_mut().enumerate() {
                    ui.separator();
                    let mut is_delete = false;
                    if word_status_edit_ui(word, ui, &mut rename, &mut is_delete, settings) {
                        *save = true;
                    }
                    if is_delete {
                        delete = Some(pos);
                    }
                }
                ui.separator();
                if ui
                    .add(Button::new("Add").text_color(settings.color_add()))
                    .clicked()
                {
                    getted.push(WordStatus::KnowPreviously);
                }
                if let Some(pos) = delete {
                    getted.remove(pos);
                    if getted.is_empty() {
                        remove_word = true;
                    }
                }
                if let Some((previous, new)) = rename {
                    words.rename_word(&previous, &new);
                }
                if self.word_to_edit != self.word {
                    words.rename_word(&self.word, &self.word_to_edit);
                    self.word = self.word_to_edit.clone();
                    return (false, true);
                }
                if remove_word {
                    words.remove_word(&self.word);
                    return (true, true);
                }
                (false, false)
            } else {
                (true, true)
            }
        }
    }

    struct AddWordsWindow {
        text: String,
        words: WordsWithContext,
        translations: String,
        known_translations: String,
        previous: Option<(String, Vec<std::ops::Range<usize>>)>,
    }

    impl WindowTrait for AddWordsWindow {
        fn create_window(&self) -> Window<'static> {
            Window::new("Add words")
                .vscroll(false)
                .fixed_size((400., 400.))
                .collapsible(false)
        }
    }

    impl AddWordsWindow {
        fn new(text: String, words: WordsWithContext) -> Self {
            AddWordsWindow {
                text,
                words,
                translations: String::new(),
                known_translations: String::new(),
                previous: None,
            }
        }

        fn ui(
            &mut self,
            ui: &mut Ui,
            search_words_window: &mut ClosableWindow<SearchWordsWindow>,
            synchronous_subtitles_window: &mut ClosableWindow<SynchronousSubtitlesWindow>,
            words: &Words,
        ) -> Option<(String, WordsToAdd, bool)> {
            ui.columns(2, |cols| {
                let ui = &mut cols[0];
                let mut action = None;
                ui.label(format!("Words remains: {}", self.words.0.len()));
                ui.label(format!("Occurences in text: {}", self.words.0[0].1.len()));
                SearchWordsWindow::find_word(
                    &mut search_words_window.0,
                    self.words.0[0].0.clone(),
                    words,
                );
                SynchronousSubtitlesWindow::change_search_string(
                    &mut synchronous_subtitles_window.0,
                    self.words.0[0].0.clone(),
                    true,
                );
                ui.separator();
                ui.horizontal(|ui| {
                    if ui.button("Skip").clicked() {
                        self.translations.clear();
                        self.known_translations.clear();
                        self.previous = Some(self.words.0.remove(0));
                    }
                    if let Some((text, ranges)) = &self.previous {
                        if ui.button(format!("Return ({})", text)).clicked() {
                            self.words.0.insert(0, (text.clone(), ranges.clone()));
                            self.previous = None;
                        }
                    } else {
                        ui.add_enabled(false, Button::new("Return previous"));
                    }
                });
                if let Some((word, to_add)) = word_to_add(
                    ui,
                    &mut self.words.0[0].0,
                    &mut self.translations,
                    &mut self.known_translations,
                ) {
                    self.translations.clear();
                    self.known_translations.clear();
                    self.previous = Some(self.words.0.remove(0));
                    action = Some((word, to_add, self.words.0.is_empty()));
                }

                let ui = &mut cols[1];
                ui.label("Context:");
                ui.separator();
                if self.words.0.is_empty() {
                    return action;
                }
                ScrollArea::vertical().max_height(200.0).show(ui, |ui| {
                    const CONTEXT_SIZE: usize = 50;
                    for range in &self.words.0[0].1 {
                        let mut start = range.start.saturating_sub(CONTEXT_SIZE);
                        let mut end = {
                            let result = range.end + CONTEXT_SIZE;
                            if result > self.text.len() {
                                self.text.len()
                            } else {
                                result
                            }
                        };
                        while start > 0 && !self.text.is_char_boundary(start) {
                            start -= 1;
                        }
                        while end < self.text.len() && !self.text.is_char_boundary(end) {
                            end += 1;
                        }
                        ui.horizontal_wrapped(|ui| {
                            ui.spacing_mut().item_spacing.x = 0.;
                            ui.label("...");
                            ui.label(&self.text[start..range.start]);
                            ui.add(egui::Label::new(&self.text[range.clone()]).strong());
                            ui.label(&self.text[range.end..end]);
                            ui.label("...");
                        });

                        ui.separator();
                    }
                });

                action
            })
        }
    }

    #[derive(Default)]
    struct AddCustomWordsWindow {
        word: String,
        translations: String,
        known_translations: String,
    }

    impl WindowTrait for AddCustomWordsWindow {
        fn create_window(&self) -> Window<'static> {
            Window::new("Add words")
                .vscroll(false)
                .fixed_size((200., 100.))
                .collapsible(false)
        }
    }

    impl AddCustomWordsWindow {
        fn ui(&mut self, ui: &mut Ui) -> Option<(String, WordsToAdd)> {
            let mut action = None;
            ui.separator();
            if let Some((word, to_add)) = word_to_add(
                ui,
                &mut self.word,
                &mut self.translations,
                &mut self.known_translations,
            ) {
                self.translations.clear();
                self.known_translations.clear();
                self.word.clear();
                action = Some((word, to_add));
            }
            action
        }
    }

    #[derive(Default)]
    struct FullStatsWindow {
        time: f64,
        attempts: TypingStats,
        word_count_by_level: BTreeMap<WordType, u64>,
    }

    impl WindowTrait for FullStatsWindow {
        fn create_window(&self) -> Window<'static> {
            Window::new("Full statistics")
                .vscroll(false)
                .fixed_size((150., 100.))
                .collapsible(false)
        }
    }

    impl FullStatsWindow {
        fn ui(&mut self, ui: &mut Ui) {
            ui.label(format!("Full working time: {}", print_time(self.time)));
            ui.separator();
            ui.label(format!(
                "Attempts: {}",
                self.attempts.right + self.attempts.wrong,
            ));
            ui.label(format!("Correct: {}", self.attempts.right,));
            ui.label(format!("Wrong: {}", self.attempts.wrong,));
            ui.separator();
            ui.label("Count of words:");
            for (kind, count) in &self.word_count_by_level {
                use WordType::*;
                match kind {
                    Known => ui.label(format!("Known: {}", count)),
                    Trash => ui.label(format!("Trash: {}", count)),
                    Level(l) => ui.label(format!("Level {}: {}", l, count)),
                    Learned => ui.label(format!("Learned: {}", count)),
                };
            }
        }
    }

    #[derive(Default)]
    struct PercentageGraphWindow {
        name: &'static str,
        values: BTreeMap<Day, Vec<f64>>,
        names: Vec<String>,
        stackplot: bool,
        moving: bool,
    }

    impl WindowTrait for PercentageGraphWindow {
        fn create_window(&self) -> Window<'static> {
            Window::new(self.name).vscroll(false).collapsible(false)
        }
    }

    impl PercentageGraphWindow {
        fn ui(&mut self, ui: &mut Ui) {
            ui.horizontal(|ui| {
                ui.checkbox(&mut self.stackplot, "Stackplot");
                ui.checkbox(&mut self.moving, "Enable moving");
            });
            use egui::plot::*;
            let mut max_value = 0.;
            let lines = (0..self.values.values().next().unwrap().len()).map(|i| {
                Line::new(Values::from_values(
                    self.values
                        .iter()
                        .map(|(day, arr)| {
                            let value = if self.stackplot {
                                arr.iter().take(i + 1).sum::<f64>()
                            } else {
                                arr[i]
                            };
                            if value > max_value {
                                max_value = value;
                            }
                            Value::new(day.0 as f64, value)
                        })
                        .collect(),
                ))
            });

            let mut plot = Plot::new(format!("percentage {}", self.moving))
                .allow_zoom(self.moving)
                .allow_drag(self.moving)
                .legend(Legend::default().position(Corner::LeftTop));
            for (line, name) in lines.zip(self.names.iter()) {
                plot = plot.line(line.name(name));
            }

            let min_day = self.values.keys().next().unwrap().0 as f64;
            let max_day = self.values.keys().rev().next().unwrap().0 as f64;
            plot = plot.polygon(
                Polygon::new(Values::from_values(vec![
                    Value::new(min_day, 0.),
                    Value::new(max_day, 0.),
                    Value::new(max_day, max_value),
                    Value::new(min_day, max_value),
                ]))
                .width(0.)
                .fill_alpha(0.005),
            );
            ui.add(plot);
        }
    }

    enum SynchronousSubtitlesWindow {
        Load {
            lang1: String,
            lang2: String,
            error1: Option<String>,
            error2: Option<String>,
        },
        View {
            search_string: String,
            whole_words_search: bool,
            found: Vec<usize>,
            position: usize,
            phrases: Vec<(Option<String>, Option<String>)>,
            update_scroll: bool,
        },
    }

    impl WindowTrait for SynchronousSubtitlesWindow {
        fn create_window(&self) -> Window<'static> {
            if matches!(self, SynchronousSubtitlesWindow::Load { .. }) {
                Window::new("Load synchronous subtitles")
                    .vscroll(true)
                    .fixed_size((300., 200.))
                    .collapsible(false)
            } else {
                Window::new("View synchronous subtitles")
                    .vscroll(false)
                    .fixed_size((400., 200.))
                    .collapsible(false)
            }
        }
    }

    impl SynchronousSubtitlesWindow {
        fn new() -> Self {
            Self::Load {
                lang1: String::new(),
                lang2: String::new(),
                error1: None,
                error2: None,
            }
        }

        fn calc_phrases(
            sub1: Vec<srtparse::Item>,
            sub2: Vec<srtparse::Item>,
        ) -> Vec<(Option<String>, Option<String>)> {
            use std::ops::RangeInclusive;

            fn convert_time(time: srtparse::Time) -> u64 {
                time.milliseconds + 1000 * (time.seconds + 60 * (time.minutes + 60 * time.hours))
            }

            fn convert(item: srtparse::Item) -> (RangeInclusive<u64>, String, bool) {
                let start = convert_time(item.start_time);
                let end = convert_time(item.end_time);
                (start..=end, item.text, false) // false means 'used'
            }

            let mut sub1: Vec<_> = sub1.into_iter().map(convert).collect();
            let mut sub2: Vec<_> = sub2.into_iter().map(convert).collect();
            let mut result = Vec::new();

            let end_times = {
                let mut result = sub1
                    .iter()
                    .enumerate()
                    .map(|(pos, x)| (pos, *x.0.end(), false))
                    .chain(
                        sub2.iter()
                            .enumerate()
                            .map(|(pos, x)| (pos, *x.0.end(), true)),
                    )
                    .collect::<Vec<_>>();
                result.sort_by_key(|x| x.1);
                result
            };

            for (pos, end, is_second) in end_times {
                #[rustfmt::skip]
                macro_rules! current { () => { if is_second { &mut sub2 } else { &mut sub1 } }; }
                #[rustfmt::skip]
                macro_rules! other { () => { if is_second { &mut sub1 } else { &mut sub2 } }; }
                if !current!()[pos].2 {
                    current!()[pos].2 = true;
                    if let Some(pos1) = other!().iter().position(|x| x.0.contains(&end) && !x.2) {
                        other!()[pos1].2 = true;
                        if is_second {
                            result.push((Some(sub1[pos1].1.clone()), Some(sub2[pos].1.clone())));
                        } else {
                            result.push((Some(sub1[pos].1.clone()), Some(sub2[pos1].1.clone())));
                        }
                    } else {
                        if is_second {
                            result.push((None, Some(sub2[pos].1.clone())));
                        } else {
                            result.push((Some(sub1[pos].1.clone()), None));
                        }
                    }
                }
            }

            result
        }

        fn change_search_string(
            this: &mut Option<Self>,
            search_string1: String,
            whole_words_search1: bool,
        ) {
            use SynchronousSubtitlesWindow::*;
            if let Some(this) = this {
                let update = if let View {
                    search_string,
                    whole_words_search,
                    ..
                } = this
                {
                    if *search_string != search_string1 {
                        *search_string = search_string1;
                        *whole_words_search = whole_words_search1;
                        true
                    } else {
                        false
                    }
                } else {
                    false
                };
                if update {
                    this.update();
                    if let View {
                        position,
                        found,
                        update_scroll,
                        ..
                    } = this
                    {
                        if found.len() > 1 {
                            *position = 1;
                            *update_scroll = true;
                        }
                    }
                }
            }
        }

        fn find_whole_word_bool(text: &str, word: &str) -> bool {
            let word = word.chars().collect::<Vec<_>>();
            #[derive(Clone)]
            enum State {
                NotAlphabetic,
                SkipCurrentWord,
                Check(usize),
            }
            use State::*;
            let mut state = NotAlphabetic;
            for c in text.chars().chain(std::iter::once('.')) {
                loop {
                    let mut to_break = true;
                    match state.clone() {
                        NotAlphabetic => {
                            if is_word_symbol(c) {
                                state = Check(0);
                                to_break = false;
                            } else {
                                // do nothing
                            }
                        }
                        SkipCurrentWord => {
                            if is_word_symbol(c) {
                                // do nothing
                            } else {
                                state = NotAlphabetic;
                            }
                        }
                        Check(pos) => {
                            if is_word_symbol(c) {
                                #[allow(clippy::collapsible_else_if)]
                                if pos == word.len() {
                                    state = SkipCurrentWord;
                                } else {
                                    // todo сделать нормально
                                    if c.to_lowercase().next().unwrap() == word[pos] {
                                        state = Check(pos + 1);
                                    } else {
                                        state = SkipCurrentWord;
                                    }
                                }
                            } else {
                                if pos == word.len() {
                                    return true;
                                }
                                state = NotAlphabetic;
                            }
                        }
                    }
                    if to_break {
                        break;
                    }
                }
            }
            false
        }

        fn find_occurence_bool(text: &str, occurence: &str) -> bool {
            text.contains(occurence)
        }

        fn update(&mut self) {
            use SynchronousSubtitlesWindow::*;
            if let View {
                search_string,
                whole_words_search,
                found,
                position,
                phrases,
                update_scroll,
            } = self
            {
                *position = 0;
                found.clear();
                found.push(0);
                if !search_string.is_empty() {
                    for (pos, text) in phrases
                        .iter()
                        .enumerate()
                        .filter_map(|(pos, x)| Some((pos, x.0.as_ref()?)))
                    {
                        let find_result = if *whole_words_search {
                            Self::find_whole_word_bool(text, search_string)
                        } else {
                            Self::find_occurence_bool(text, search_string)
                        };

                        if find_result {
                            found.push(pos);
                        }
                    }
                }
                *update_scroll = true;
                if found.len() > 1 {
                    *position = 1;
                }
            }
        }

        fn ui(&mut self, ui: &mut Ui, settings: &Settings) {
            use SynchronousSubtitlesWindow::*;
            let mut update = None;
            let mut update_search = false;
            match self {
                Load {
                    lang1,
                    lang2,
                    error1,
                    error2,
                } => {
                    if ui.button("Use these subtitles").clicked() {
                        let sub1 = match srtparse::from_str(&lang1) {
                            Ok(sub1) => Some(sub1),
                            Err(err) => {
                                *error1 = Some(format!("{:#?}", err));
                                None
                            }
                        };
                        let sub2 = match srtparse::from_str(&lang2) {
                            Ok(sub2) => Some(sub2),
                            Err(err) => {
                                *error2 = Some(format!("{:#?}", err));
                                None
                            }
                        };
                        update = sub1.zip(sub2);
                    }
                    if error1.is_some() || error2.is_some() {
                        ui.separator();
                        if let Some(error1) = error1 {
                            ui.horizontal_wrapped(|ui| {
                                ui.spacing_mut().item_spacing.x = 0.;
                                ui.add(
                                    Label::new("Left Error: ")
                                        .text_color(settings.color_error())
                                        .monospace(),
                                );
                                ui.monospace(&**error1);
                            });
                        }
                        if let Some(error2) = error2 {
                            ui.horizontal_wrapped(|ui| {
                                ui.spacing_mut().item_spacing.x = 0.;
                                ui.add(
                                    Label::new("Right Error: ")
                                        .text_color(settings.color_error())
                                        .monospace(),
                                );
                                ui.monospace(&**error2);
                            });
                        }
                    }
                    ui.separator();
                    ui.columns(2, |cols| {
                        cols[0].text_edit_multiline(lang1);
                        cols[1].text_edit_multiline(lang2);
                    });
                }
                View {
                    search_string,
                    whole_words_search,
                    found,
                    position,
                    phrases,
                    update_scroll: update_scroll_origin,
                } => {
                    let mut update_scroll = *update_scroll_origin;
                    *update_scroll_origin = false;
                    if ui
                        .add(
                            TextEdit::singleline(search_string)
                                .hint_text("Type here to find word..."),
                        )
                        .changed()
                    {
                        update_search = true;
                    }
                    ui.horizontal(|ui| {
                        if ui
                            .checkbox(whole_words_search, "Search by whole words")
                            .changed()
                        {
                            update_search = true;
                        }
                        ui.separator();
                        if ui.add_enabled(*position > 1, Button::new("◀")).clicked() {
                            *position -= 1;
                            update_scroll = true;
                        }
                        if ui
                            .add_enabled(*position + 1 < found.len(), Button::new("▶"))
                            .clicked()
                        {
                            *position += 1;
                            update_scroll = true;
                        }
                        ui.label(format!("{}/{}", *position, found.len() - 1));
                    });
                    ui.separator();
                    ScrollArea::vertical().max_height(200.0).show(ui, |ui| {
                        Grid::new("view_grid")
                            .spacing([4.0, 4.0])
                            .max_col_width(150.)
                            .striped(true)
                            .show(ui, |ui| {
                                for (pos, (a, b)) in phrases.iter().enumerate() {
                                    ui.label(format!("{}", pos + 1));
                                    if !found.is_empty() && found[*position] == pos {
                                        let response = if let Some(text) = a {
                                            if *position == 0 {
                                                ui.label(text)
                                            } else {
                                                ui.add(Label::new(&text).strong())
                                            }
                                        } else {
                                            ui.label("-")
                                        };
                                        if update_scroll {
                                            response.scroll_to_me(Align::Center);
                                        }
                                    } else {
                                        if let Some(text) = a {
                                            ui.label(text);
                                        } else {
                                            ui.label("-");
                                        }
                                    }
                                    if let Some(text) = b {
                                        ui.label(text);
                                    } else {
                                        ui.label("-");
                                    }
                                    ui.end_row();
                                }
                            });
                    });
                }
            }
            if update_search {
                self.update();
            }
            if let Some((sub1, sub2)) = update {
                let phrases = Self::calc_phrases(sub1, sub2);
                *self = View {
                    search_string: String::new(),
                    whole_words_search: false,
                    found: vec![0],
                    position: 0,
                    phrases,
                    update_scroll: false,
                };
                self.update();
            }
        }
    }

    struct GithubDayData {
        attempts: u64,
        time: f64,
        new_unknown_words_count: u64,
    }

    struct GithubActivityWindow {
        max_day: Day,
        min_day: Day,

        data_by_day: BTreeMap<Day, GithubDayData>,
        max_value: GithubDayData,
        min_value: GithubDayData,

        show: u8,

        show_day: Day,
        drag_delta: f32,
    }

    impl WindowTrait for GithubActivityWindow {
        fn create_window(&self) -> Window<'static> {
            Window::new("Activity")
                .vscroll(false)
                .collapsible(false)
                .resizable(false)
        }
    }

    fn date_from_day(day: Day) -> chrono::Date<chrono::Utc> {
        use chrono::TimeZone;
        chrono::Utc
            .timestamp(day.0 as i64 * 24 * 60 * 60 + 3600, 0)
            .date()
    }

    impl GithubActivityWindow {
        fn new(stats: &Statistics, today: Day) -> Self {
            let data_by_day: BTreeMap<Day, GithubDayData> = stats
                .by_day
                .iter()
                .map(|(d, x)| {
                    (
                        *d,
                        GithubDayData {
                            attempts: x.attempts.right + x.attempts.wrong,
                            time: x.working_time,
                            new_unknown_words_count: x.new_unknown_words_count,
                        },
                    )
                })
                .collect();
            let min_value = GithubDayData {
                attempts: data_by_day.values().map(|x| x.attempts).min().unwrap(),
                time: data_by_day
                    .values()
                    .map(|x| x.time)
                    .min_by(|x, y| x.partial_cmp(y).unwrap())
                    .unwrap(),
                new_unknown_words_count: data_by_day
                    .values()
                    .map(|x| x.new_unknown_words_count)
                    .min()
                    .unwrap(),
            };
            let max_value = GithubDayData {
                attempts: data_by_day.values().map(|x| x.attempts).max().unwrap(),
                time: data_by_day
                    .values()
                    .map(|x| x.time)
                    .max_by(|x, y| x.partial_cmp(y).unwrap())
                    .unwrap(),
                new_unknown_words_count: data_by_day
                    .values()
                    .map(|x| x.new_unknown_words_count)
                    .max()
                    .unwrap(),
            };
            Self {
                min_day: *data_by_day.keys().next().unwrap(),
                max_day: today,

                data_by_day,
                max_value,
                min_value,

                show: 0,

                show_day: today,
                drag_delta: 0.,
            }
        }

        fn get_normalized_value(&self, day: Day) -> Option<f64> {
            fn normalize(min: f64, max: f64, v: f64) -> f64 {
                (v - min) / (max - min)
            }

            match self.show {
                0 => self.data_by_day.get(&day).map(|x| {
                    normalize(
                        self.min_value.attempts as f64,
                        self.max_value.attempts as f64,
                        x.attempts as f64,
                    )
                }),
                1 => self
                    .data_by_day
                    .get(&day)
                    .map(|x| normalize(self.min_value.time, self.max_value.time, x.time)),
                _ => self.data_by_day.get(&day).map(|x| {
                    normalize(
                        self.min_value.new_unknown_words_count as f64,
                        self.max_value.new_unknown_words_count as f64,
                        x.new_unknown_words_count as f64,
                    )
                }),
            }
        }

        fn get_value_text(&self, day: Day) -> Option<String> {
            self.data_by_day.get(&day).map(|x| {
                format!(
                    "Attempts: {}\nTime: {}\nNew words: {}\nTime for 1 attempt: {:.1}s",
                    x.attempts,
                    print_time(x.time),
                    x.new_unknown_words_count,
                    x.time / x.attempts as f64
                )
            })
        }

        fn ui(&mut self, ui: &mut Ui, settings: &Settings) {
            ui.horizontal(|ui| {
                ui.label("Show data about: ");
                ui.selectable_value(&mut self.show, 0, "Attempts");
                ui.selectable_value(&mut self.show, 1, "Working time");
                ui.selectable_value(&mut self.show, 2, "New words");
            });
            ui.separator();

            let size = 8.;
            let margin = 1.5;
            let weeks = 53;
            let days = 7;

            let month_size = ui.fonts()[TextStyle::Body].row_height();
            let weekday_size = 30.;

            let desired_size = egui::vec2(
                2. * margin + weeks as f32 * (size + margin) + weekday_size,
                2. * margin + days as f32 * (size + margin) + month_size * 2.,
            );
            let (rect, response) = ui.allocate_exact_size(desired_size, Sense::drag());

            self.drag_delta += response.drag_delta().x;
            let offset_weeks = (self.drag_delta / (size + margin)) as i64;
            let show_day = Day((self.show_day.0 as i64 - offset_weeks * 7) as u64);

            use chrono::Datelike;
            let today_date = date_from_day(show_day);
            let today_week = today_date.weekday().number_from_monday() - 1;
            let today_pos = 52 * 7 + today_week;

            let min = rect.min + egui::vec2(margin + weekday_size, margin + month_size);
            let size2 = egui::vec2(size, size);
            let margin2 = egui::vec2(margin, margin) / 2.;
            let stroke_hovered = Stroke::new(1., settings.color_github_month());
            let stroke_month = Stroke::new(0.5, settings.color_github_month());
            let stroke_year = Stroke::new(1., settings.color_github_year());
            let left_1 = egui::vec2(-margin / 2., -margin / 2.);
            let right_1 = egui::vec2(size + margin / 2., -margin / 2.);
            let right_2 = egui::vec2(size + margin / 2., -margin / 2. - month_size);
            let down_1 = egui::vec2(-margin / 2., size + margin / 2.);
            let end_line = egui::vec2(size + margin / 2., size + margin / 2.);
            let end_line2 = egui::vec2(size + margin / 2., size + margin / 2. + month_size);
            let mut month_pos = BTreeMap::new();
            let mut year_pos = BTreeMap::new();
            for i in 0..weeks {
                for j in 0..days {
                    let pos = i * 7 + j;
                    let day = Day(show_day.0 - today_pos as u64 + pos);
                    let date = date_from_day(day);

                    if j + 1 == days {
                        month_pos
                            .entry((date.month(), date.year()))
                            .or_insert_with(Vec::new)
                            .push(i);
                    }
                    if j == 0 {
                        year_pos.entry(date.year()).or_insert_with(Vec::new).push(i);
                    }

                    let pos =
                        min + egui::vec2(i as f32 * (size + margin), j as f32 * (size + margin));

                    if i + 1 != weeks {
                        let pos_right = (i + 1) * 7 + j;
                        let day_right = Day(show_day.0 - today_pos as u64 + pos_right);
                        let date_right = date_from_day(day_right);

                        if date_right.year() != date.year() {
                            if j == 0 {
                                ui.painter()
                                    .line_segment([pos + right_2, pos + end_line2], stroke_year);
                            } else if j + 1 == days {
                                ui.painter()
                                    .line_segment([pos + right_1, pos + end_line2], stroke_year);
                            } else {
                                ui.painter()
                                    .line_segment([pos + right_1, pos + end_line], stroke_year);
                            }
                        } else if date_right.month() != date.month() {
                            if j + 1 == days {
                                ui.painter()
                                    .line_segment([pos + right_1, pos + end_line2], stroke_month);
                            } else {
                                ui.painter()
                                    .line_segment([pos + right_1, pos + end_line], stroke_month);
                            }
                        }
                    }

                    if j == 0 {
                        ui.painter()
                            .line_segment([pos + left_1, pos + right_1], stroke_month);
                    } else if j + 1 == days {
                        ui.painter()
                            .line_segment([pos + down_1, pos + end_line], stroke_month);
                    }

                    if j + 1 != days {
                        let pos_down = i * 7 + (j + 1);
                        let day_down = Day(show_day.0 - today_pos as u64 + pos_down);
                        let date_down = date_from_day(day_down);

                        if date_down.year() != date.year() {
                            ui.painter()
                                .line_segment([pos + down_1, pos + end_line], stroke_year);
                        } else if date_down.month() != date.month() {
                            ui.painter()
                                .line_segment([pos + down_1, pos + end_line], stroke_month);
                        }
                    }

                    let color = if day.0 < self.min_day.0 || day.0 > self.max_day.0 {
                        settings.color_github_zero()
                    } else if let Some(value) = self.get_normalized_value(day) {
                        let zero_color = settings.color_github_zero();
                        let min_color = settings.color_github_low();
                        let max_color = settings.color_github_high();

                        let value = if settings.white_theme {
                            (value as f32).powf(0.7)
                        } else {
                            (value as f32).powf(0.71)
                        };

                        if value < 0.1 {
                            let value = value / 0.1;
                            Color32::from(lerp(
                                Rgba::from(zero_color)..=Rgba::from(min_color),
                                value,
                            ))
                        } else {
                            let value = (value - 0.1) / (1.0 - 0.1);
                            Color32::from(lerp(
                                Rgba::from(min_color)..=Rgba::from(max_color),
                                value,
                            ))
                        }
                    } else {
                        ui.visuals().faint_bg_color
                    };

                    let mut rect = egui::Rect::from_min_max(pos, pos + size2);

                    ui.painter().rect_filled(rect, 0., color);

                    if let Some(pos) = response.hover_pos() {
                        rect.min -= margin2;
                        rect.max += margin2;
                        if rect.contains(pos) && !response.dragged() {
                            let data = self.get_value_text(day);
                            let text = format!("{}-{}-{}", date.year(), date.month(), date.day())
                                + if data.is_some() { "\n" } else { "" }
                                + &data.unwrap_or_else(String::new);
                            egui::show_tooltip_text(ui.ctx(), egui::Id::new("date tooltip"), text);
                            ui.painter()
                                .rect(rect, 0., Color32::TRANSPARENT, stroke_hovered);
                        }
                    }
                }
            }
            for ((month, _), pos) in &month_pos {
                if pos.len() < 3 {
                    continue;
                }
                let pos = pos.iter().sum::<u64>() as f32 / pos.len() as f32;
                let pos = min + egui::vec2(pos * (size + margin), 7. * (size + margin));
                let month = match month {
                    1 => "Jan",
                    2 => "Feb",
                    3 => "Mar",
                    4 => "Apr",
                    5 => "May",
                    6 => "Jun",
                    7 => "Jul",
                    8 => "Aug",
                    9 => "Sep",
                    10 => "Oct",
                    11 => "Nov",
                    12 => "Dec",
                    _ => unreachable!(),
                };
                ui.painter().text(
                    pos,
                    Align2::CENTER_TOP,
                    month,
                    TextStyle::Body,
                    ui.visuals().text_color(),
                );
            }
            for (year, pos) in &year_pos {
                if pos.len() < 3 {
                    continue;
                }
                let pos = pos.iter().sum::<u64>() as f32 / pos.len() as f32;
                let pos = min + egui::vec2(pos * (size + margin), -month_size - margin);
                let year = year.to_string();
                ui.painter().text(
                    pos,
                    Align2::CENTER_TOP,
                    year,
                    TextStyle::Body,
                    ui.visuals().text_color(),
                );
            }
            ui.painter().text(
                min + egui::vec2(-weekday_size, size / 2.),
                Align2::LEFT_CENTER,
                "Mon",
                TextStyle::Body,
                ui.visuals().text_color(),
            );
            ui.painter().text(
                min + egui::vec2(-weekday_size, size * 7. + size / 2.),
                Align2::LEFT_CENTER,
                "Sun",
                TextStyle::Body,
                ui.visuals().text_color(),
            );
        }
    }

    struct ToTypeToday {
        all_words: Vec<String>,
        current_batch: Vec<String>,
    }

    // Это окно нельзя закрыть
    struct LearnWordsWindow {
        to_type_repeat: Vec<(String, u64)>,
        to_type_new: Vec<(String, u64)>,

        to_type_today: Option<ToTypeToday>,
        current: LearnWords,
    }

    enum LearnWords {
        None,
        Choose {
            all_repeat: usize,
            all_new: usize,
            n_repeat: usize,
            n_new: usize,
        },
        Typing {
            word: String,
            word_by_hint: Option<String>,
            correct_answer: WordsToLearn,
            words_to_type: Vec<String>,
            words_to_guess: Vec<String>,
            max_types: u8,
            gain_focus: bool,
        },
        Checked {
            word: String,
            known_words: Vec<String>,
            typed: Vec<String>,
            to_repeat: Vec<String>,
            result: Vec<TypedWord>,
            max_types: u8,
            gain_focus: bool,
        },
    }

    struct TypedWord {
        correct: bool,
        translation: String,
        typed: String,
    }

    fn select_with_translations(
        word: &str,
        words: &Words,
        today: Day,
        type_count: &[LearnType],
        mut f: impl FnMut(&str),
    ) {
        f(word);
        if let Some(variants) = words.0.get(word) {
            for i in variants {
                if i.can_learn_today(today, type_count) {
                    if let WordStatus::ToLearn { translation, .. } = i {
                        f(translation);
                    }
                }
            }
        }
    }

    impl LearnWordsWindow {
        fn new(words: &Words, today: Day, type_count: &[LearnType], rng: &mut Rand) -> Self {
            let mut result = Self {
                to_type_repeat: Vec::new(),
                to_type_new: Vec::new(),

                to_type_today: None,
                current: LearnWords::None,
            };
            result.update(words, today, type_count, rng);
            result
        }

        fn cancel_learning(&mut self) {
            self.to_type_today = None;
            self.current = LearnWords::Choose {
                all_repeat: self.to_type_repeat.len(),
                all_new: self.to_type_new.len(),
                n_repeat: 30,
                n_new: 15,
            };
        }

        fn pick_current_type(
            &mut self,
            words: &Words,
            today: Day,
            type_count: &[LearnType],
            rng: &mut Rand,
        ) {
            if let Some(to_type_today) = &mut self.to_type_today {
                to_type_today
                    .all_words
                    .retain(|x| words.can_learn_today(x, today, type_count));
            }

            loop {
                if self.to_type_repeat.is_empty()
                    && self.to_type_new.is_empty()
                    && self
                        .to_type_today
                        .as_ref()
                        .map(|x| x.current_batch.is_empty() && x.all_words.is_empty())
                        .unwrap_or(true)
                {
                    self.current = LearnWords::None;
                    return;
                }

                if self
                    .to_type_today
                    .as_ref()
                    .map(|x| x.all_words.is_empty())
                    .unwrap_or(false)
                {
                    self.to_type_today = None;
                }

                if let Some(to_type_today) = &mut self.to_type_today {
                    if to_type_today.current_batch.is_empty() {
                        let (hint_words, guess_words): (Vec<_>, Vec<_>) = to_type_today
                            .all_words
                            .iter()
                            .cloned()
                            .partition(|x| words.has_hint(x, type_count));

                        if hint_words.is_empty() {
                            to_type_today.current_batch = guess_words;
                        } else {
                            to_type_today.current_batch = hint_words;
                        }

                        to_type_today.current_batch.shuffle(rng);
                    }

                    let word = to_type_today.current_batch.remove(0);
                    if !words.is_learned(&word) {
                        let max_types = words.max_attempts_remains(&word, today, type_count);
                        let result = words.get_word_to_learn(&word, today, type_count);
                        let words_to_type: Vec<String> = (0..result.words_to_type.len())
                            .map(|_| String::new())
                            .collect();
                        let words_to_guess: Vec<String> = (0..result.words_to_guess.len())
                            .map(|_| String::new())
                            .collect();
                        if words_to_type.is_empty() && words_to_guess.is_empty() {
                            to_type_today.all_words.retain(|x| *x != word);
                            to_type_today.current_batch.retain(|x| *x != word);
                        } else {
                            self.current = LearnWords::Typing {
                                word,
                                word_by_hint: (!words_to_type.is_empty()).then(String::new),
                                correct_answer: result,
                                words_to_type,
                                max_types,
                                words_to_guess,
                                gain_focus: true,
                            };
                            return;
                        }
                    } else {
                        to_type_today.all_words.retain(|x| *x != word);
                        to_type_today.current_batch.retain(|x| *x != word);
                    }
                } else {
                    self.cancel_learning();
                    return;
                }
            }
        }

        fn update(&mut self, words: &Words, today: Day, type_count: &[LearnType], rng: &mut Rand) {
            let (repeat, new) = words.get_words_to_learn_today(today, type_count);

            self.to_type_repeat.clear();
            for i in repeat {
                let overdue = words.max_overdue_days(&i, today, type_count);
                self.to_type_repeat.push((i, overdue));
            }
            self.to_type_repeat.sort_by_key(|x| std::cmp::Reverse(x.1));

            self.to_type_new.clear();
            for i in new {
                let overdue = words.max_overdue_days(&i, today, type_count);
                self.to_type_new.push((i, overdue));
            }
            self.to_type_new.sort_by_key(|x| std::cmp::Reverse(x.1));

            self.pick_current_type(words, today, type_count, rng);
        }

        #[allow(clippy::too_many_arguments)]
        fn ui(
            &mut self,
            ctx: &CtxRef,
            words: &mut Words,
            today: Day,
            day_stats: &mut DayStatistics,
            settings: &Settings,
            save: &mut bool,
            rng: &mut Rand,
        ) {
            let mut cancel = false;
            egui::Window::new("Learn words")
                .fixed_size((300., 0.))
                .collapsible(false)
                .vscroll(false)
                .show(ctx, |ui| match &mut self.current {
                    LearnWords::None => {
                        ui.label("🎉🎉🎉 Everything is learned for today! 🎉🎉🎉");
                    }
                    LearnWords::Choose {
                        all_repeat,
                        all_new,
                        n_repeat,
                        n_new,
                    } => {
                        ui.label("Choose words to work with now.");
                        ui.horizontal(|ui| {
                            ui.label("Old words to repeat: ");
                            ui.add(
                                egui::DragValue::new(n_repeat)
                                    .clamp_range(0..=*all_repeat)
                                    .speed(1.0),
                            );
                            ui.label(format!("/{}", all_repeat))
                        });
                        ui.horizontal(|ui| {
                            ui.label("New words to learn: ");
                            ui.add(
                                egui::DragValue::new(n_new)
                                    .clamp_range(0..=*all_new)
                                    .speed(1.0),
                            );
                            ui.label(format!("/{}", all_new))
                        });
                        if ui.button("Choose").clicked() {
                            let to_type_repeat = &mut self.to_type_repeat;
                            let to_type_new = &mut self.to_type_new;

                            self.to_type_today = Some({
                                let mut result = BTreeSet::new();

                                while (n_repeat == all_repeat || result.len() < *n_repeat)
                                    && !to_type_repeat.is_empty()
                                {
                                    let first = to_type_repeat[0].clone();
                                    select_with_translations(
                                        &first.0,
                                        words,
                                        today,
                                        &settings.type_count,
                                        |word| {
                                            to_type_repeat.retain(|x| x.0 != word);
                                            result.insert(word.to_string());
                                        },
                                    );
                                }

                                *n_repeat = result.len();

                                while (n_new == all_new || result.len() < *n_repeat + *n_new)
                                    && !to_type_new.is_empty()
                                {
                                    let first = to_type_new[0].clone();
                                    select_with_translations(
                                        &first.0,
                                        words,
                                        today,
                                        &settings.type_count,
                                        |word| {
                                            to_type_new.retain(|x| x.0 != word);
                                            result.insert(word.to_string());
                                        },
                                    );
                                }

                                ToTypeToday {
                                    all_words: result.into_iter().collect(),
                                    current_batch: Vec::new(),
                                }
                            });

                            self.pick_current_type(words, today, &settings.type_count, rng);
                        }
                    }
                    LearnWords::Typing {
                        word,
                        word_by_hint,
                        correct_answer,
                        words_to_type,
                        words_to_guess,
                        gain_focus,
                        max_types,
                    } => {
                        let len = self.to_type_today.as_ref().unwrap().all_words.len();
                        ui.with_layout(Layout::right_to_left(), |ui| {
                            if ui.button("❌").clicked() {
                                cancel = true;
                            }
                            ui.with_layout(Layout::left_to_right(), |ui| {
                                ui.label(format!("Words remains: {}.", len));
                            });
                        });
                        ui.label(format!("This word attempts remains: {}.", max_types));
                        ui.separator();

                        let mut data = InputFieldData::new(settings, &mut *gain_focus);

                        if let Some(word_by_hint) = word_by_hint {
                            ui.label("Word:");
                            InputField::Hint.ui(ui, &mut data, word_by_hint, word, settings);
                            ui.separator();
                        } else {
                            ui.add(Label::new(&word).heading().strong());
                        }

                        for i in &mut correct_answer.known_words {
                            ui.add_enabled(false, egui::TextEdit::singleline(i));
                        }
                        for (hint, i) in correct_answer
                            .words_to_type
                            .iter()
                            .zip(words_to_type.iter_mut())
                        {
                            InputField::Hint.ui(ui, &mut data, i, hint, settings);
                        }
                        for (i, correct) in words_to_guess
                            .iter_mut()
                            .zip(correct_answer.words_to_guess.iter())
                        {
                            InputField::Input.ui(ui, &mut data, i, correct, settings);
                        }

                        if input_field_button(ui, "Check", &mut data) {
                            // Register just typed words
                            for answer in &correct_answer.words_to_type {
                                words.register_attempt(
                                    word,
                                    answer,
                                    true,
                                    today,
                                    day_stats,
                                    &settings.type_count,
                                );
                            }

                            let mut result = Vec::new();
                            let mut answers = correct_answer.words_to_guess.clone();
                            let mut corrects = Vec::new();
                            for typed in &*words_to_guess {
                                if let Some(position) = answers.iter().position(|x| x == typed) {
                                    corrects.push(answers.remove(position));
                                }
                            }

                            for typed in &*words_to_guess {
                                let (answer, correct) = if let Some(position) =
                                    corrects.iter().position(|x| x == typed)
                                {
                                    (corrects.remove(position), true)
                                } else {
                                    (answers.remove(0), false)
                                };

                                result.push(TypedWord {
                                    correct,
                                    translation: answer,
                                    typed: typed.clone(),
                                });
                            }

                            if result.is_empty() {
                                for typed_word in result.iter_mut() {
                                    words.register_attempt(
                                        word,
                                        &typed_word.translation,
                                        typed_word.correct,
                                        today,
                                        day_stats,
                                        &settings.type_count,
                                    );
                                }
                                self.pick_current_type(words, today, &settings.type_count, rng);
                                *save = true;
                            } else {
                                self.current = LearnWords::Checked {
                                    word: word.clone(),
                                    known_words: correct_answer.known_words.clone(),
                                    typed: correct_answer.words_to_type.clone(),
                                    to_repeat: (0..result.len()).map(|_| String::new()).collect(),
                                    result,
                                    max_types: *max_types,
                                    gain_focus: true,
                                };
                            }
                        }
                    }
                    LearnWords::Checked {
                        word,
                        known_words,
                        typed,
                        result,
                        to_repeat,
                        max_types,
                        gain_focus,
                    } => {
                        let len = self.to_type_today.as_ref().unwrap().all_words.len();
                        ui.with_layout(Layout::right_to_left(), |ui| {
                            if ui.button("❌").clicked() {
                                cancel = true;
                            }
                            ui.with_layout(Layout::left_to_right(), |ui| {
                                ui.label(format!("Words remains: {}.", len));
                            });
                        });
                        ui.label(format!("This word attempts remains: {}.", max_types));
                        ui.separator();
                        ui.add(Label::new(&word).heading().strong());

                        let mut data = InputFieldData::new(settings, &mut *gain_focus);

                        for i in known_words {
                            ui.add_enabled(false, egui::TextEdit::singleline(i));
                        }

                        for i in typed {
                            with_green_color(
                                ui,
                                |ui| {
                                    ui.add_enabled(false, egui::TextEdit::singleline(i));
                                },
                                settings,
                            );
                        }

                        for word in result.iter_mut() {
                            InputField::Checked(&mut word.correct).ui(
                                ui,
                                &mut data,
                                &mut word.typed,
                                &word.translation,
                                settings,
                            );
                        }

                        if result.iter().any(|x| !x.correct) {
                            ui.separator();
                            ui.label("Correction of mistakes:");
                        }

                        for (word, to_repeat) in result.iter_mut().zip(to_repeat.iter_mut()) {
                            if !word.correct {
                                InputField::Hint.ui(
                                    ui,
                                    &mut data,
                                    to_repeat,
                                    &word.translation,
                                    settings,
                                );
                            }
                        }

                        if input_field_button(ui, "Next", &mut data) {
                            for typed_word in result.iter_mut() {
                                words.register_attempt(
                                    word,
                                    &typed_word.translation,
                                    typed_word.correct,
                                    today,
                                    day_stats,
                                    &settings.type_count,
                                );
                            }
                            self.pick_current_type(words, today, &settings.type_count, rng);
                            *save = true;
                        }
                    }
                });
            if cancel {
                self.update(words, today, &settings.type_count, rng);
                self.cancel_learning();
            }
        }
    }

    enum InputField<'a> {
        Hint,
        Input,
        Checked(&'a mut bool),
    }

    struct FocusThing {
        last_response: Option<Response>,
        last_enabled_response: Option<Response>,
        give_next_focus: u8,
    }

    struct InputFieldData<'a> {
        f: FocusThing,
        focus_gained: bool,
        gain_focus: &'a mut bool,
        settings: &'a Settings,
        is_empty: bool,

        next_enabled: bool,
    }

    impl Drop for FocusThing {
        fn drop(&mut self) {
            if self.give_next_focus == 1 {
                if let Some(last) = &mut self.last_enabled_response {
                    last.request_focus();
                }
            }
        }
    }

    impl<'a> InputFieldData<'a> {
        fn new(settings: &'a Settings, gain_focus: &'a mut bool) -> InputFieldData<'a> {
            Self {
                f: FocusThing {
                    last_response: None,
                    last_enabled_response: None,
                    give_next_focus: 0,
                },
                focus_gained: false,
                gain_focus,
                settings,
                is_empty: false,

                next_enabled: true,
            }
        }

        fn process_text(&self, input: &mut String, should_be: &str) {
            if self.settings.use_keyboard_layout {
                self.settings.keyboard_layout.change(should_be, input);
            }
        }

        fn process_focus(&mut self, response: Response, input: &InputState, allow_gain: bool) {
            if self.f.give_next_focus == 1 && self.next_enabled {
                response.request_focus();
                self.f.give_next_focus = 2;
            }
            if response.has_focus()
                && input.events.iter().any(|x| {
                    if let Event::Key { key, pressed, .. } = x {
                        *key == Key::Backspace && *pressed
                    } else {
                        false
                    }
                })
                && self.is_empty
            {
                if let Some(last_response) = &self.f.last_response {
                    last_response.request_focus();
                }
            }
            if response.lost_focus()
                && input.events.iter().any(|x| {
                    if let Event::Key { key, pressed, .. } = x {
                        *key == Key::Enter && *pressed
                    } else {
                        false
                    }
                })
                && self.f.give_next_focus == 0
            {
                self.f.give_next_focus = 1;
            }
            if !self.focus_gained && *self.gain_focus && self.next_enabled && allow_gain {
                response.request_focus();
                self.focus_gained = true;
                *self.gain_focus = false;
            }
            if response.enabled() {
                self.f.last_enabled_response = Some(response.clone());
            }
            self.f.last_response = Some(response);
        }
    }

    fn input_field_button(ui: &mut Ui, text: &str, data: &mut InputFieldData) -> bool {
        data.is_empty = true;
        let response = ui.add_enabled(data.next_enabled, Button::new(text));
        let result = response.clicked();
        data.process_focus(response, ui.input(), true);
        result
    }

    impl InputField<'_> {
        fn ui(
            &mut self,
            ui: &mut Ui,
            data: &mut InputFieldData,
            input: &mut String,
            should_be: &str,
            settings: &Settings,
        ) {
            use InputField::*;
            match self {
                Hint => {
                    data.is_empty = input.is_empty();
                    let response = if input == should_be {
                        with_green_color(
                            ui,
                            |ui| {
                                ui.add_enabled(
                                    data.next_enabled,
                                    egui::TextEdit::singleline(input)
                                        .hint_text(format!(" {}", should_be)),
                                )
                            },
                            settings,
                        )
                    } else {
                        ui.add_enabled(
                            data.next_enabled,
                            egui::TextEdit::singleline(input).hint_text(format!(" {}", should_be)),
                        )
                    };
                    data.process_text(input, should_be);
                    data.process_focus(response, ui.input(), true);
                    data.next_enabled &= input == should_be;
                }
                Input => {
                    data.is_empty = input.is_empty();
                    let response =
                        ui.add_enabled(data.next_enabled, egui::TextEdit::singleline(input));
                    data.process_text(input, should_be);
                    data.process_focus(response, ui.input(), true);
                }
                Checked(checked) => {
                    ui.with_layout(Layout::right_to_left(), |ui| {
                        let response = ui.button("Invert");
                        if response.clicked() {
                            **checked = !**checked;
                        }
                        data.process_focus(response, ui.input(), false);
                        if **checked {
                            ui.label(format!("✅ {}", should_be));
                            with_green_color(
                                ui,
                                |ui| {
                                    ui.add_enabled(false, egui::TextEdit::singleline(input));
                                },
                                settings,
                            );
                        } else {
                            ui.label(format!("❌ {}", should_be));
                            with_red_color(
                                ui,
                                |ui| {
                                    ui.add_enabled(false, egui::TextEdit::singleline(input));
                                },
                                settings,
                            );
                        }
                    });
                }
            }
        }
    }

    fn word_to_add(
        ui: &mut Ui,
        word: &mut String,
        translations: &mut String,
        known_translations: &mut String,
    ) -> Option<(String, WordsToAdd)> {
        let mut action = None;
        ui.horizontal(|ui| {
            ui.label("Word:");
            ui.text_edit_singleline(word);
        });
        ui.separator();
        ui.horizontal(|ui| {
            if ui.button("Know this word").clicked() {
                action = Some((word.clone(), WordsToAdd::KnowPreviously));
            }
            if ui.button("Trash word").clicked() {
                action = Some((word.clone(), WordsToAdd::TrashWord));
            }
        });
        ui.separator();
        ui.label("Translations:");
        ui.add(TextEdit::multiline(translations).desired_rows(2));
        ui.separator();
        ui.label("Known translations:");
        ui.add(TextEdit::multiline(known_translations).desired_rows(2));
        if ui.button("Add these translations").clicked() {
            action = Some((
                word.clone(),
                WordsToAdd::ToLearn {
                    learned: known_translations
                        .split('\n')
                        .map(|x| x.to_string())
                        .filter(|x| !x.is_empty())
                        .collect(),
                    translations: translations
                        .split('\n')
                        .map(|x| x.to_string())
                        .filter(|x| !x.is_empty())
                        .collect(),
                },
            ));
        }
        action
    }

    fn with_color<Res>(
        ui: &mut Ui,
        color1: Color32,
        color2: Color32,
        color3: Color32,
        f: impl FnOnce(&mut Ui) -> Res,
    ) -> Res {
        let previous = ui.visuals().clone();
        ui.visuals_mut().selection.stroke.color = color1;
        ui.visuals_mut().widgets.inactive.bg_stroke.color = color2;
        ui.visuals_mut().widgets.inactive.bg_stroke.width = 1.0;
        ui.visuals_mut().widgets.hovered.bg_stroke.color = color3;
        let result = f(ui);
        *ui.visuals_mut() = previous;
        result
    }

    fn with_green_color<Res>(
        ui: &mut Ui,
        f: impl FnOnce(&mut Ui) -> Res,
        settings: &Settings,
    ) -> Res {
        with_color(
            ui,
            settings.color_green_field_1(),
            settings.color_green_field_2(),
            settings.color_green_field_3(),
            f,
        )
    }

    fn with_red_color<Res>(
        ui: &mut Ui,
        f: impl FnOnce(&mut Ui) -> Res,
        settings: &Settings,
    ) -> Res {
        with_color(
            ui,
            settings.color_red_field_1(),
            settings.color_red_field_2(),
            settings.color_red_field_3(),
            f,
        )
    }

    fn word_status_show_ui(word: &WordStatus, ui: &mut Ui) {
        use WordStatus::*;
        match word {
            KnowPreviously => ui.label("Known"),
            TrashWord => ui.label("Trash"),
            ToLearn {
                translation,
                last_learn,
                current_level,
                current_count,
                stats,
            } => {
                ui.label(format!("To learn: '{}'", translation));
                ui.label(format!("Attempts: +{}, -{}", stats.right, stats.wrong));
                ui.label(format!("Last learned: {} day", last_learn.0));
                ui.label(format!("Current level: {}", current_level));
                ui.label(format!("Current correct writes: {}", current_count))
            }
            Learned { translation, stats } => {
                ui.label(format!("Learned: '{}'", translation));
                ui.label(format!("Attempts: +{}, -{}", stats.right, stats.wrong))
            }
        };
    }

    pub trait ComboBoxChoosable {
        fn variants() -> &'static [&'static str];
        fn get_number(&self) -> usize;
        fn set_number(&mut self, number: usize);
    }

    impl ComboBoxChoosable for WordStatus {
        fn variants() -> &'static [&'static str] {
            &["Known", "Trash", "To learn", "Learned."]
        }
        fn get_number(&self) -> usize {
            use WordStatus::*;
            match self {
                KnowPreviously => 0,
                TrashWord => 1,
                ToLearn { .. } => 2,
                Learned { .. } => 3,
            }
        }
        fn set_number(&mut self, number: usize) {
            use WordStatus::*;
            *self = match number {
                0 => KnowPreviously,
                1 => TrashWord,
                2 => {
                    if let Learned { translation, stats } = self {
                        ToLearn {
                            translation: translation.to_string(),
                            stats: *stats,
                            last_learn: Day(0),
                            current_level: 0,
                            current_count: 0,
                        }
                    } else {
                        ToLearn {
                            translation: String::new(),
                            stats: TypingStats { right: 0, wrong: 0 },
                            last_learn: Day(0),
                            current_level: 0,
                            current_count: 0,
                        }
                    }
                }
                3 => {
                    if let ToLearn {
                        translation, stats, ..
                    } = self
                    {
                        Learned {
                            translation: translation.to_string(),
                            stats: *stats,
                        }
                    } else {
                        Learned {
                            translation: String::new(),
                            stats: TypingStats { right: 0, wrong: 0 },
                        }
                    }
                }
                _ => unreachable!(),
            };
        }
    }

    fn word_status_edit_ui(
        word: &mut WordStatus,
        ui: &mut Ui,
        rename: &mut Option<(String, String)>,
        is_delete: &mut bool,
        settings: &Settings,
    ) -> bool {
        use WordStatus::*;

        let mut changed = false;

        let mut current_type = word.get_number();
        let previous_type = current_type;

        ui.with_layout(Layout::right_to_left(), |ui| {
            if ui
                .add(Button::new("Delete").text_color(settings.color_delete()))
                .clicked()
            {
                *is_delete = true;
            }
            ui.with_layout(Layout::left_to_right(), |ui| {
                for (pos, name) in WordStatus::variants().iter().enumerate().take(2) {
                    ui.selectable_value(&mut current_type, pos, *name);
                }
            });
        });

        ui.horizontal(|ui| {
            for (pos, name) in WordStatus::variants().iter().enumerate().skip(2) {
                ui.selectable_value(&mut current_type, pos, *name);
            }
        });

        if current_type != previous_type {
            word.set_number(current_type);
            changed = true;
        }

        if let ToLearn {
            translation, stats, ..
        }
        | Learned { translation, stats } = word
        {
            let previous = translation.clone();

            ui.text_edit_singleline(translation);

            if previous != *translation {
                *rename = Some((previous, translation.clone()));
            }

            ui.horizontal(|ui| {
                ui.label("Right attempts: ");
                let response = ui.add(
                    egui::DragValue::new(&mut stats.right)
                        .clamp_range(0..=100)
                        .speed(1.0),
                );
                if response.changed() {
                    changed = true;
                }
            });
            ui.horizontal(|ui| {
                ui.label("Wrong attempts: ");
                let response = ui.add(
                    egui::DragValue::new(&mut stats.wrong)
                        .clamp_range(0..=100)
                        .speed(1.0),
                );
                if response.changed() {
                    changed = true;
                }
            });
        }
        if let ToLearn {
            last_learn,
            current_level,
            current_count,
            ..
        } = word
        {
            ui.horizontal(|ui| {
                ui.label("Last learn: ");
                let response = ui.add(
                    egui::DragValue::new(&mut last_learn.0)
                        .clamp_range(0..=100_000)
                        .speed(1.0),
                );
                if response.changed() {
                    changed = true;
                }
            });
            ui.horizontal(|ui| {
                ui.label("Current level: ");
                let response = ui.add(
                    egui::DragValue::new(current_level)
                        .clamp_range(0..=100)
                        .speed(1.0),
                );
                if response.changed() {
                    changed = true;
                }
            });
            ui.horizontal(|ui| {
                ui.label("Current correct writes: ");
                let response = ui.add(
                    egui::DragValue::new(current_count)
                        .clamp_range(0..=100)
                        .speed(1.0),
                );
                if response.changed() {
                    changed = true;
                }
            });
        }
        changed
    }
}

struct PauseDetector {
    last_mouse_position: (f32, f32),
    pausing: bool,
    time: f64,

    last_time: f64,
    time_without_pauses: f64,
}

impl PauseDetector {
    fn new(time_today: f64) -> Self {
        Self {
            last_mouse_position: (0., 0.),
            pausing: false,
            time: now(),
            last_time: now(),
            time_without_pauses: time_today,
        }
    }

    fn is_paused(&mut self, settings: &Settings, input: &egui::InputState) -> bool {
        let current_mouse_position = {
            let p = input.pointer.hover_pos().unwrap_or_default();
            (p.x, p.y)
        };
        let mouse_offset = (self.last_mouse_position.0 - current_mouse_position.0).abs()
            + (self.last_mouse_position.1 - current_mouse_position.1).abs();
        let mouse_not_moving = mouse_offset < 0.01;
        let mouse_not_clicking = !input.pointer.any_down();
        let keyboard_not_typing = input.keys_down.is_empty();

        self.last_mouse_position = current_mouse_position;
        let now = now();
        if !(self.pausing && now - self.time > settings.time_to_pause) {
            self.time_without_pauses += now - self.last_time;
        }
        self.last_time = now;

        if mouse_not_moving && keyboard_not_typing && mouse_not_clicking {
            if self.pausing {
                now - self.time > settings.time_to_pause
            } else {
                self.pausing = true;
                self.time = now;
                false
            }
        } else {
            self.pausing = false;
            false
        }
    }

    fn get_working_time(&mut self) -> &mut f64 {
        &mut self.time_without_pauses
    }
}

use eframe::{egui, epi};

pub struct TemplateApp {
    rng: Rand,
    today: Day,
    pause_detector: PauseDetector,
    program: gui::Program,
    init: bool,
}

impl Default for TemplateApp {
    fn default() -> Self {
        #[cfg(not(target_arch = "wasm32"))]
        color_backtrace::install();

        #[cfg(target_arch = "wasm32")]
        std::panic::set_hook(Box::new(console_error_panic_hook::hook));

        fn current_day(hour_offset: f64) -> Day {
            Day(((now() / 60. / 60. + hour_offset) / 24.) as _)
        }

        let mut rng = Rand::seed_from_u64(now() as u64);

        let (words, settings, stats) = gui::Program::load();
        let today = current_day(timezone_offset_hours());

        let mut pause_detector = PauseDetector::new(
            stats
                .by_day
                .get(&today)
                .map(|x| x.working_time)
                .unwrap_or(0.),
        );

        let program = gui::Program::new(
            words,
            settings,
            stats,
            today,
            *pause_detector.get_working_time(),
            &mut rng,
        );

        Self {
            rng,
            today,
            pause_detector,
            program,
            init: false,
        }
    }
}

impl epi::App for TemplateApp {
    fn name(&self) -> &str {
        "Learn Words"
    }

    fn update(&mut self, ctx: &egui::CtxRef, _: &mut epi::Frame<'_>) {
        if !self.init {
            self.init = true;

            if self.program.get_settings().white_theme {
                ctx.set_visuals(egui::Visuals::light());
            } else {
                ctx.set_visuals(egui::Visuals::dark());
            }

            ctx.set_pixels_per_point(self.program.get_settings().dpi);
        }

        let mut fill = ctx.style().visuals.extreme_bg_color;
        if !cfg!(target_arch = "wasm32") {
            // Native: WrapApp uses a transparent window, so let's show that off:
            // NOTE: the OS compositor assumes "normal" blending, so we need to hack it:
            let [r, g, b, _] = fill.to_array();
            fill = egui::Color32::from_rgba_premultiplied(r, g, b, 180);
        }
        let frame = egui::Frame::none().fill(fill);
        egui::CentralPanel::default().frame(frame).show(ctx, |_| {});

        let paused = self
            .pause_detector
            .is_paused(self.program.get_settings(), ctx.input());
        self.program.ui(
            ctx,
            self.today,
            self.pause_detector.get_working_time(),
            &mut self.rng,
            paused,
        );
    }
}

// ----------------------------------------------------------------------------

fn timezone_offset_hours() -> f64 {
    #[cfg(not(target_arch = "wasm32"))]
    {
        use chrono::offset::Offset;
        use chrono::offset::TimeZone;
        use chrono::Local;
        Local.timestamp(0, 0).offset().fix().local_minus_utc() as f64 / 3600.
    }

    #[cfg(target_arch = "wasm32")]
    {
        -js_sys::Date::new_0().get_timezone_offset() / 60.
    }
}

pub fn now() -> f64 {
    #[cfg(not(target_arch = "wasm32"))]
    {
        use std::time::SystemTime;

        let time = SystemTime::now()
            .duration_since(SystemTime::UNIX_EPOCH)
            .unwrap_or_else(|e| panic!("{}", e));
        time.as_secs_f64()
    }

    #[cfg(target_arch = "wasm32")]
    {
        js_sys::Date::now() / 1000.0
    }
}

#[cfg(target_arch = "wasm32")]
pub fn download_as_file(text: &str) {
    use wasm_bindgen::JsCast;

    let window = web_sys::window().unwrap();
    let document = window.document().unwrap();

    let elem = document.create_element("a").unwrap();
    let data = format!(
        "data:text/plain;charset=utf-8,{}",
        String::from(js_sys::encode_uri_component(text))
    );
    elem.set_attribute("href", &data).unwrap();
    elem.set_attribute("download", "local.data").unwrap();

    let elem = elem.unchecked_into::<web_sys::HtmlElement>();

    elem.style().set_property("display", "none").unwrap();

    let body = document.body().expect("2");

    body.append_child(&elem).expect("3");
    document.set_body(Some(&body));

    elem.click();

    body.remove_child(&elem).expect("6");
    document.set_body(Some(&body));
}

// ----------------------------------------------------------------------------

#[cfg(target_arch = "wasm32")]
use eframe::wasm_bindgen::{self, prelude::*};

#[cfg(target_arch = "wasm32")]
#[wasm_bindgen]
pub fn start(canvas_id: &str) -> Result<(), eframe::wasm_bindgen::JsValue> {
    let app = TemplateApp::default();
    eframe::start_web(canvas_id, Box::new(app))
}

#[cfg(target_arch = "wasm32")]
fn main() {}

#[cfg(not(target_arch = "wasm32"))]
fn main() {
    let app = TemplateApp::default();
    let native_options = eframe::NativeOptions::default();
    eframe::run_native(Box::new(app), native_options);
}
