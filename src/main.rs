use macroquad::prelude::*;
use serde::*;
use std::collections::BTreeMap;
use std::collections::BTreeSet;

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
        today.0 - last_learn.0 >= self.wait_days as u64
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

        /// Количество изучений слова, при поиске того что надо печатать, проходим по всему массиву
        learns: Vec<LearnType>,

        /// Количество learns, которое уже преодолено
        current_level: u8,

        /// Статистика
        stats: TypingStats,
    },

    /// Мы знаем это слово
    Learned {
        translation: String,

        /// Статистика
        stats: TypingStats,
    },
}

impl WordStatus {
    fn register_attempt(&mut self, correct: bool, today: Day, day_stats: &mut DayStatistics) {
        use WordStatus::*;
        match self {
            KnowPreviously | TrashWord | Learned { .. } => unreachable!(),
            ToLearn {
                stats,
                learns,
                last_learn,
                translation,
                current_level,
            } => {
                if correct {
                    stats.right += 1;
                    day_stats.attempts.right += 1;
                } else {
                    stats.wrong += 1;
                    day_stats.attempts.wrong += 1;
                }

                let mut registered = false;

                if correct {
                    let mut other_learns = Vec::new();
                    std::mem::swap(&mut other_learns, learns);
                    *learns = other_learns
                        .into_iter()
                        .filter_map(|mut learn| {
                            if learn.can_learn_today(*last_learn, today) && !registered {
                                registered = true;
                                if learn.count > 1 {
                                    learn.count -= 1;
                                    Some(learn)
                                } else {
                                    *last_learn = today;
                                    *current_level += 1;
                                    None
                                }
                            } else {
                                Some(learn)
                            }
                        })
                        .collect();

                    if learns.is_empty() {
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

    fn can_learn_today(&self, today: Day) -> bool {
        if let WordStatus::ToLearn {
            last_learn, learns, ..
        } = self
        {
            learns
                .iter()
                .any(|learn| learn.can_learn_today(*last_learn, today))
        } else {
            false
        }
    }
}

/// Все слова в программе
#[derive(Default, Serialize, Deserialize, Clone, Debug)]
pub struct Words(BTreeMap<String, Vec<WordStatus>>);

enum WordsToAdd {
    KnowPreviously,
    TrashWord,
    ToLearn { translations: Vec<String> },
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
        settings: &Settings,
        today: Day,
        day_stats: &mut DayStatistics,
    ) {
        use WordsToAdd::*;
        let entry = self.0.entry(word.clone()).or_insert_with(Vec::new);
        match info {
            KnowPreviously => entry.push(WordStatus::KnowPreviously),
            TrashWord => entry.push(WordStatus::TrashWord),
            ToLearn { translations } => {
                for translation in &translations {
                    entry.push(WordStatus::ToLearn {
                        translation: translation.clone(),
                        last_learn: today,
                        learns: settings.type_count.clone(),
                        current_level: 0,
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
                            learns: settings.type_count.clone(),
                            current_level: 0,
                            stats: Default::default(),
                        });
                }
            }
        }
    }

    fn is_learned(&self, word: &str) -> bool {
        for i in self.0.get(word).unwrap() {
            if matches!(i, WordStatus::ToLearn { .. }) {
                return false;
            }
        }
        true
    }

    fn get_word_to_learn(&self, word: &str, today: Day) -> WordsToLearn {
        let mut known_words = Vec::new();
        let mut words_to_type = Vec::new();
        let mut words_to_guess = Vec::new();
        for i in self.0.get(word).unwrap() {
            if let WordStatus::ToLearn {
                translation,
                last_learn,
                learns,
                ..
            } = i
            {
                for learn in learns {
                    if learn.can_learn_today(*last_learn, today) {
                        if learn.show_word {
                            words_to_type.push(translation.clone());
                        } else {
                            words_to_guess.push(translation.clone());
                        }
                        break;
                    } else {
                        known_words.push(translation.clone());
                    }
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

    fn get_words_to_learn_today(&self, today: Day) -> Vec<String> {
        self.0
            .iter()
            .filter(|(_, statuses)| statuses.iter().any(|x| x.can_learn_today(today)))
            .map(|(word, _)| word.clone())
            .collect()
    }

    fn register_attempt(
        &mut self,
        word: &str,
        translation: &str,
        correct: bool,
        today: Day,
        day_stats: &mut DayStatistics,
    ) {
        for i in self.0.get_mut(word).unwrap() {
            if i.has_translation(translation) {
                i.register_attempt(correct, today, day_stats);
                return;
            }
        }
        unreachable!()
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
}

fn get_words_subtitles(subtitles: &str) -> Result<Vec<String>, srtparse::ReaderError> {
    let subtitles = srtparse::from_str(subtitles)?;
    let text = subtitles
        .into_iter()
        .map(|x| x.text)
        .collect::<Vec<_>>()
        .join("\n");

    Ok(get_words(&text))
}

fn get_words(text: &str) -> Vec<String> {
    text.to_lowercase()
        .chars()
        .map(|c| {
            if !c.is_alphabetic() && c != '\'' && c != '-' {
                ' '
            } else {
                c
            }
        })
        .collect::<String>()
        .split(' ')
        .filter(|x| !x.is_empty())
        .map(|x| x.to_string())
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect::<Vec<_>>()
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct Settings {
    type_count: Vec<LearnType>,
    time_to_pause: f64,
    use_keyboard_layout: bool,
    keyboard_layout: KeyboardLayout,
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

        dbg!(&result);

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
            .map(|x| if let Some(c) = lang.get(&x) { *c } else { x })
            .collect();
    }
}

impl Default for Settings {
    fn default() -> Self {
        Settings {
            type_count: vec![
                LearnType::show(0, 1),
                LearnType::guess(0, 1),
                LearnType::guess(2, 5),
                LearnType::guess(7, 5),
                LearnType::guess(20, 5),
            ],
            time_to_pause: 15.,
            use_keyboard_layout: false,
            keyboard_layout: Default::default(),
        }
    }
}

fn read_clipboard() -> Option<String> {
    miniquad::clipboard::get(unsafe { get_internal_gl().quad_context })
}

fn write_clipboard(s: &str) {
    miniquad::clipboard::set(unsafe { get_internal_gl().quad_context }, s)
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
        settings_window: ClosableWindow<SettingsWindow>,
    }

    impl Program {
        pub fn new(
            words: Words,
            settings: Settings,
            stats: Statistics,
            today: Day,
            working_time: f64,
        ) -> Self {
            let learn_window = LearnWordsWindow::new(&words, today);
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
                settings_window: Default::default(),
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
            std::fs::write("learn_words.data", self.save_to_string(today, working_time)).unwrap();
        }

        pub fn load() -> (Words, Settings, Statistics) {
            std::fs::read_to_string("learn_words.data")
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

        pub fn ui(&mut self, ctx: &CtxRef, today: Day, working_time: f64) {
            TopBottomPanel::top("top").show(ctx, |ui| {
                menu::bar(ui, |ui| {
                    menu::menu(ui, "Data", |ui| {
                        if ui.button("Export to clipboard").clicked() {
                            write_clipboard(&self.save_to_string(today, working_time));
                        }
                        if ui.button("Import").clicked() {
                            self.import_window = ClosableWindow::new(ImportWindow::new());
                        }
                        ui.separator();
                        if ui.button("Save").clicked() {
                            self.save(today, working_time);
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
                    });
                    menu::menu(ui, "Statistics", |ui| {
                        if ui.button("Full").clicked() {
                            self.full_stats_window = ClosableWindow::new(FullStatsWindow {
                                attempts: self.words.calculate_attempts_statistics(),
                                word_count_by_level: self.words.calculate_word_statistics(),
                            });
                        }
                        if ui.button("GitHub-like").clicked() {
                            self.open_activity(today, working_time);
                        }
                        ui.separator();
                        if ui.button("Attempts by day").clicked() {
                            self.update_day_statistics(today, working_time);
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
                                });
                        }
                        if ui.button("Time by day").clicked() {
                            self.update_day_statistics(today, working_time);
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
                                });
                        }
                        if ui.button("Words by day").clicked() {
                            self.update_day_statistics(today, working_time);
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
                                });
                        }
                    });
                    if ui.button("Settings").clicked() {
                        self.settings_window =
                            ClosableWindow::new(SettingsWindow::new(&self.settings));
                    }
                    if ui.button("About").clicked() {}
                });
            });

            self.learn_window.ui(
                ctx,
                &mut self.words,
                today,
                &mut self.stats.by_day.entry(today).or_default(),
                &self.settings,
            );

            let window = &mut self.load_text_window;
            let known_words = &self.known_words;
            let add_words_window = &mut self.add_words_window;
            window.ui(ctx, |t, ui| {
                if let Some(words) = t.ui(ui, known_words) {
                    *add_words_window = ClosableWindow::new(AddWordsWindow::new(words));
                    true
                } else {
                    false
                }
            });

            let window = &mut self.import_window;
            let words = &mut self.words;
            let settings = &mut self.settings;
            let stats = &mut self.stats;
            let closed = window.ui(ctx, |t, ui| {
                if let Some((words1, settings1, stats1)) = t.ui(ui) {
                    *words = words1;
                    *settings = settings1;
                    *stats = stats1;
                    true
                } else {
                    false
                }
            });
            if closed {
                self.learn_window.update(&self.words, today);
            }

            let window = &mut self.settings_window;
            let settings = &mut self.settings;
            window.ui(ctx, |t, ui| {
                t.ui(ui, settings);
                false
            });

            let window = &mut self.add_words_window;
            let settings = &self.settings;
            let words = &mut self.words;
            let stats = &mut self.stats;
            let closed = window.ui(ctx, |t, ui| {
                if let Some((word, to_add, close)) = t.ui(ui) {
                    words.add_word(
                        word,
                        to_add,
                        settings,
                        today,
                        stats.by_day.entry(today).or_default(),
                    );
                    close
                } else {
                    false
                }
            });
            if closed {
                self.learn_window.update(&self.words, today);
            }

            let window = &mut self.add_custom_words_window;
            let settings = &self.settings;
            let words = &mut self.words;
            let stats = &mut self.stats;
            let closed = window.ui(ctx, |t, ui| {
                if let Some((word, to_add)) = t.ui(ui) {
                    words.add_word(
                        word,
                        to_add,
                        settings,
                        today,
                        stats.by_day.entry(today).or_default(),
                    );
                }
                false
            });
            if closed {
                self.learn_window.update(&self.words, today);
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
                t.ui(ui);
                false
            });

            egui::TopBottomPanel::bottom("bottom").show(ctx, |ui| {
                let today = &self.stats.by_day.entry(today).or_default();
                ui.monospace(format!(
                    "Working time: {:6} | Attempts: {:4} | New words: {:4}",
                    print_time(working_time),
                    today.attempts.right + today.attempts.wrong,
                    today.new_unknown_words_count,
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
        text: Result<String, String>,
    }

    impl WindowTrait for LoadTextWindow {
        fn create_window(&self) -> Window<'static> {
            Window::new(if self.load_subtitles {
                "Words from subs"
            } else {
                "Words from text"
            })
            .scroll(false)
            .fixed_size((200., 100.))
            .collapsible(false)
        }
    }

    impl LoadTextWindow {
        fn new(load_subtitles: bool) -> Self {
            Self {
                load_subtitles,
                subtitles_error: None,
                text: read_clipboard().ok_or_else(String::new),
            }
        }

        fn ui(&mut self, ui: &mut Ui, known_words: &BTreeSet<String>) -> Option<Vec<String>> {
            let mut action = None;
            ui.horizontal(|ui| {
                if ui.button("Use this text").clicked() {
                    let text = self.text.as_ref().unwrap_or_else(|x| x);

                    let words = if self.load_subtitles {
                        match get_words_subtitles(&text) {
                            Ok(words) => Some(words),
                            Err(error) => {
                                self.subtitles_error = Some(format!("{:#?}", error));
                                None
                            }
                        }
                    } else {
                        Some(get_words(&text))
                    };
                    if let Some(words) = words {
                        let words = words
                            .into_iter()
                            .filter(|x| !known_words.contains(x))
                            .collect();
                        action = Some(words);
                    }
                }
                if ui.button("Update clipboard").clicked() {
                    self.text = read_clipboard().ok_or_else(String::new);
                }
            });
            ui.separator();
            match &mut self.text {
                Ok(text) => {
                    text_preview(ui, text);
                }
                Err(text) => {
                    ui.text_edit_multiline(text);
                }
            }
            if let Some(error) = &self.subtitles_error {
                ui.separator();
                ui.horizontal_wrapped(|ui| {
                    ui.spacing_mut().item_spacing.x = 0.;
                    ui.add(Label::new("Error: ").text_color(Color32::RED).monospace());
                    ui.monospace(error);
                });
            }
            action
        }
    }

    struct ImportWindow {
        text: Result<String, String>,
        error: Option<String>,
    }

    impl WindowTrait for ImportWindow {
        fn create_window(&self) -> Window<'static> {
            Window::new("Import data")
                .scroll(false)
                .fixed_size((200., 100.))
                .collapsible(false)
        }
    }

    impl ImportWindow {
        fn new() -> Self {
            Self {
                text: read_clipboard().ok_or_else(String::new),
                error: None,
            }
        }

        fn ui(&mut self, ui: &mut Ui) -> Option<(Words, Settings, Statistics)> {
            let mut action = None;
            ui.horizontal(|ui| {
                if ui.button("Use this text").clicked() {
                    let text = self.text.as_ref().unwrap_or_else(|x| x);

                    match Program::load_from_string(text) {
                        Ok(result) => action = Some(result),
                        Err(error) => {
                            self.error = Some(format!("{:#?}", error));
                        }
                    }
                }
                if ui.button("Update clipboard").clicked() {
                    self.text = read_clipboard().ok_or_else(String::new);
                }
            });
            ui.separator();
            match &mut self.text {
                Ok(text) => {
                    text_preview(ui, text);
                }
                Err(text) => {
                    ui.text_edit_multiline(text);
                }
            }
            if let Some(error) = &self.error {
                ui.separator();
                ui.horizontal_wrapped(|ui| {
                    ui.spacing_mut().item_spacing.x = 0.;
                    ui.add(Label::new("Error: ").text_color(Color32::RED).monospace());
                    ui.monospace(error);
                });
            }
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
                .scroll(false)
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

        fn ui(&mut self, ui: &mut Ui, settings: &mut Settings) {
            ui.horizontal(|ui| {
                ui.label("Inaction time for pause: ");
                ui.add(
                    egui::DragValue::new(&mut settings.time_to_pause)
                        .speed(0.1)
                        .clamp_range(0.0..=100.0)
                        .min_decimals(0)
                        .max_decimals(2),
                );
            });

            if !self.want_to_use_keyboard_layout && settings.use_keyboard_layout {
                self.want_to_use_keyboard_layout = true;
            }
            ui.checkbox(
                &mut self.want_to_use_keyboard_layout,
                "Use automatical change of keyboard layout",
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
                                ui.add(Label::new("Error: ").text_color(Color32::RED).monospace());
                                ui.monospace(err);
                            });
                        }
                    }
                }
            } else {
                settings.use_keyboard_layout = false;
            }
        }
    }

    struct AddWordsWindow {
        words: Vec<String>,
        translations: String,
    }

    impl WindowTrait for AddWordsWindow {
        fn create_window(&self) -> Window<'static> {
            Window::new("Add words")
                .scroll(false)
                .fixed_size((200., 100.))
                .collapsible(false)
        }
    }

    impl AddWordsWindow {
        fn new(words: Vec<String>) -> Self {
            AddWordsWindow {
                words,
                translations: String::new(),
            }
        }

        fn ui(&mut self, ui: &mut Ui) -> Option<(String, WordsToAdd, bool)> {
            let mut action = None;
            ui.label(format!("Words remains: {}", self.words.len()));
            ui.separator();
            if let Some((word, to_add)) =
                word_to_add(ui, &mut self.words[0], &mut self.translations)
            {
                self.translations.clear();
                self.words.remove(0);
                action = Some((word, to_add, self.words.is_empty()));
            }
            action
        }
    }

    #[derive(Default)]
    struct AddCustomWordsWindow {
        word: String,
        translations: String,
    }

    impl WindowTrait for AddCustomWordsWindow {
        fn create_window(&self) -> Window<'static> {
            Window::new("Add words")
                .scroll(false)
                .fixed_size((200., 100.))
                .collapsible(false)
        }
    }

    impl AddCustomWordsWindow {
        fn ui(&mut self, ui: &mut Ui) -> Option<(String, WordsToAdd)> {
            let mut action = None;
            ui.separator();
            if let Some((word, to_add)) = word_to_add(ui, &mut self.word, &mut self.translations) {
                self.translations.clear();
                self.word.clear();
                action = Some((word, to_add));
            }
            action
        }
    }

    #[derive(Default)]
    struct FullStatsWindow {
        attempts: TypingStats,
        word_count_by_level: BTreeMap<WordType, u64>,
    }

    impl WindowTrait for FullStatsWindow {
        fn create_window(&self) -> Window<'static> {
            Window::new("Full statistics")
                .scroll(false)
                .fixed_size((150., 100.))
                .collapsible(false)
        }
    }

    impl FullStatsWindow {
        fn ui(&mut self, ui: &mut Ui) {
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
    }

    impl WindowTrait for PercentageGraphWindow {
        fn create_window(&self) -> Window<'static> {
            Window::new(self.name).scroll(false).collapsible(false)
        }
    }

    impl PercentageGraphWindow {
        fn ui(&mut self, ui: &mut Ui) {
            ui.checkbox(&mut self.stackplot, "Stackplot");
            use egui::plot::*;
            let lines: Vec<_> = (0..self.values.values().next().unwrap().len())
                .map(|i| {
                    Line::new(Values::from_values(
                        self.values
                            .iter()
                            .map(|(day, arr)| {
                                Value::new(
                                    day.0 as f64,
                                    if self.stackplot {
                                        arr.iter().take(i + 1).sum::<f64>()
                                    } else {
                                        arr[i]
                                    },
                                )
                            })
                            .collect(),
                    ))
                })
                .collect();

            let mut plot = Plot::new("percentage")
                .allow_zoom(false)
                .allow_drag(false)
                .legend(Legend::default().position(Corner::LeftTop));
            for (line, name) in lines.into_iter().zip(self.names.iter()) {
                plot = plot.line(line.name(name));
            }
            ui.add(plot);
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
            Window::new("Activity").scroll(false).collapsible(false)
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
                    "Attempts: {}\nTime: {}\nNew words: {}",
                    x.attempts,
                    print_time(x.time),
                    x.new_unknown_words_count
                )
            })
        }

        fn ui(&mut self, ui: &mut Ui) {
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
            let stroke_hovered = Stroke::new(1., Color32::WHITE);
            let stroke_month = Stroke::new(0.5, Color32::WHITE);
            let stroke_year = Stroke::new(1., Color32::RED);
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
                        ui.visuals().faint_bg_color
                    } else if let Some(value) = self.get_normalized_value(day) {
                        Color32::from(lerp(
                            Rgba::from(ui.visuals().faint_bg_color)..=Rgba::from(Color32::GREEN),
                            (((value as f32) + 0.2) / 1.2).powi(2),
                        ))
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

    // Это окно нельзя закрыть
    struct LearnWordsWindow {
        /// То что надо ввести несколько раз повторяется, слово повторяется максимальное число из всех под-слов что с ним связано. Если слово уже известно, то надо
        to_type_today: Vec<String>,
        current: LearnWords,
    }

    enum LearnWords {
        None,
        Typing {
            word: String,
            correct_answer: WordsToLearn,
            words_to_type: Vec<String>,
            words_to_guess: Vec<String>,
            gain_focus: bool,
        },
        Checked {
            word: String,
            known_words: Vec<String>,
            words_to_type: Vec<Result<String, (String, String)>>,
            words_to_guess: Vec<Result<String, (String, String)>>,
            gain_focus: bool,
        },
    }

    impl LearnWordsWindow {
        fn new(words: &Words, today: Day) -> Self {
            let mut result = Self {
                to_type_today: Vec::new(),
                current: LearnWords::None,
            };
            result.update(words, today);
            result
        }

        fn pick_current_type(&mut self, words: &Words, today: Day) {
            loop {
                if self.to_type_today.is_empty() {
                    self.current = LearnWords::None;
                    return;
                }

                let position = ::rand::random::<usize>() % self.to_type_today.len();
                let word = &self.to_type_today[position];
                if !words.is_learned(word) {
                    let result = words.get_word_to_learn(word, today);
                    let words_to_type: Vec<String> = (0..result.words_to_type.len())
                        .map(|_| String::new())
                        .collect();
                    let words_to_guess: Vec<String> = (0..result.words_to_guess.len())
                        .map(|_| String::new())
                        .collect();
                    if words_to_type.is_empty() && words_to_guess.is_empty() {
                        self.to_type_today.remove(position);
                    } else {
                        self.current = LearnWords::Typing {
                            word: word.clone(),
                            correct_answer: result,
                            words_to_type,
                            words_to_guess,
                            gain_focus: true,
                        };
                        return;
                    }
                } else {
                    self.to_type_today.remove(position);
                }
            }
        }

        fn update(&mut self, words: &Words, today: Day) {
            self.to_type_today = words.get_words_to_learn_today(today);
            self.pick_current_type(words, today);
        }

        fn ui(
            &mut self,
            ctx: &CtxRef,
            words: &mut Words,
            today: Day,
            day_stats: &mut DayStatistics,
            settings: &Settings,
        ) {
            egui::Window::new("Learn words")
                .fixed_size((300., 100.))
                .collapsible(false)
                .scroll(false)
                .show(ctx, |ui| match &mut self.current {
                    LearnWords::None => {
                        ui.label("🎉🎉🎉 Everything is learned for today! 🎉🎉🎉");
                    }
                    LearnWords::Typing {
                        word,
                        correct_answer,
                        words_to_type,
                        words_to_guess,
                        gain_focus,
                    } => {
                        ui.label(format!("Words remains: {}", self.to_type_today.len()));
                        ui.label(format!("Word: {}", word));

                        let mut focus_gained = false;

                        for i in &mut correct_answer.known_words {
                            ui.add(egui::TextEdit::singleline(i).enabled(false));
                        }
                        for (hint, i) in correct_answer
                            .words_to_type
                            .iter()
                            .zip(words_to_type.iter_mut())
                        {
                            let response = ui.add(egui::TextEdit::singleline(i).hint_text(hint));
                            if settings.use_keyboard_layout {
                                settings.keyboard_layout.change(hint, i);
                            }
                            if !focus_gained && *gain_focus {
                                response.request_focus();
                                focus_gained = true;
                                *gain_focus = false;
                            }
                        }
                        for (i, correct) in words_to_guess
                            .iter_mut()
                            .zip(correct_answer.words_to_guess.iter())
                        {
                            let response = ui.add(egui::TextEdit::singleline(i));
                            if settings.use_keyboard_layout {
                                settings.keyboard_layout.change(correct, i);
                            }
                            if !focus_gained && *gain_focus {
                                response.request_focus();
                                focus_gained = true;
                                *gain_focus = false;
                            }
                        }
                        if ui.button("Check").clicked() {
                            let mut words_to_type_result = Vec::new();
                            let mut words_to_guess_result = Vec::new();
                            for (answer, i) in correct_answer
                                .words_to_type
                                .iter()
                                .zip(words_to_type.iter_mut())
                            {
                                let correct = answer == i;
                                words.register_attempt(word, answer, correct, today, day_stats);
                                if correct {
                                    words_to_type_result.push(Ok(answer.clone()));
                                } else {
                                    words_to_guess_result.push(Err((answer.clone(), i.clone())));
                                }
                            }
                            let mut answers = correct_answer.words_to_guess.clone();
                            let mut corrects = Vec::new();
                            for typed in &*words_to_guess {
                                if let Some(position) = answers.iter().position(|x| x == typed) {
                                    corrects.push(answers.remove(position));
                                }
                            }

                            for typed in &*words_to_guess {
                                if let Some(position) = corrects.iter().position(|x| x == typed) {
                                    words.register_attempt(
                                        word,
                                        &corrects[position],
                                        true,
                                        today,
                                        day_stats,
                                    );
                                    corrects.remove(position);
                                    words_to_type_result.push(Ok(typed.clone()));
                                } else {
                                    let answer = answers.remove(0);
                                    words.register_attempt(word, &answer, false, today, day_stats);
                                    words_to_guess_result.push(Err((answer, typed.clone())));
                                }
                            }

                            self.current = LearnWords::Checked {
                                word: word.clone(),
                                known_words: correct_answer.known_words.clone(),
                                words_to_type: words_to_type_result,
                                words_to_guess: words_to_guess_result,
                                gain_focus: true,
                            };
                        }
                    }
                    LearnWords::Checked {
                        word,
                        known_words,
                        words_to_type,
                        words_to_guess,
                        gain_focus,
                    } => {
                        ui.label(format!("Words remains: {}", self.to_type_today.len()));
                        ui.label(format!("Word: {}", word));

                        for i in known_words {
                            ui.add(egui::TextEdit::singleline(i).enabled(false));
                        }

                        Grid::new("matrix").striped(true).show(ui, |ui| {
                            for word in words_to_type.iter_mut().chain(words_to_guess.iter_mut()) {
                                match word {
                                    Ok(word) => {
                                        with_green_color(ui, |ui| {
                                            ui.add(egui::TextEdit::singleline(word).enabled(false));
                                        });
                                        ui.label(format!("✅ {}", word));
                                    }
                                    Err((answer, word)) => {
                                        with_red_color(ui, |ui| {
                                            ui.add(egui::TextEdit::singleline(word).enabled(false));
                                        });
                                        ui.label(format!("❌ {}", answer));
                                    }
                                }
                                ui.end_row();
                            }
                        });

                        let response = ui.add(Button::new("Next"));
                        if *gain_focus {
                            response.request_focus();
                            *gain_focus = false;
                        }
                        if response.clicked() {
                            self.pick_current_type(words, today);
                        }
                    }
                });
        }
    }

    fn word_to_add(
        ui: &mut Ui,
        word: &mut String,
        translations: &mut String,
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
        ui.text_edit_multiline(translations);
        if ui.button("Add these translations").clicked() {
            action = Some((
                word.clone(),
                WordsToAdd::ToLearn {
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

    fn with_green_color<Res>(ui: &mut Ui, f: impl FnOnce(&mut Ui) -> Res) -> Res {
        with_color(
            ui,
            Color32::GREEN,
            Color32::from_rgb_additive(0, 128, 0),
            Color32::from_rgb_additive(128, 255, 128),
            f,
        )
    }

    fn with_red_color<Res>(ui: &mut Ui, f: impl FnOnce(&mut Ui) -> Res) -> Res {
        with_color(
            ui,
            Color32::RED,
            Color32::from_rgb_additive(128, 0, 0),
            Color32::from_rgb_additive(255, 128, 128),
            f,
        )
    }

    fn text_preview(ui: &mut Ui, text: &str) {
        if text.len() > 200 {
            ui.monospace(format!("{}...", text.chars().take(200).collect::<String>()));
            ui.separator();
            ui.label(format!("Total size: {:.1} KB", text.len() as f64 / 1024.0));
        } else {
            ui.label(&*text);
        }
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
            last_mouse_position: mouse_position(),
            pausing: false,
            time: get_time(),
            last_time: get_time(),
            time_without_pauses: time_today,
        }
    }

    fn is_paused(&mut self, settings: &Settings) -> bool {
        let current_mouse_position = mouse_position();
        let mouse_offset = (self.last_mouse_position.0 - current_mouse_position.0).abs()
            + (self.last_mouse_position.1 - current_mouse_position.1).abs();
        let mouse_not_moving = mouse_offset < 0.01;
        let keyboard_not_typing = get_last_key_pressed().is_none();

        self.last_mouse_position = current_mouse_position;
        let now = get_time();
        if !(self.pausing && now - self.time > settings.time_to_pause) {
            self.time_without_pauses += now - self.last_time;
        }
        self.last_time = now;

        if mouse_not_moving && keyboard_not_typing {
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

    fn get_working_time(&self) -> f64 {
        self.time_without_pauses
    }
}

fn window_conf() -> Conf {
    Conf {
        window_title: "Learn Words".to_owned(),
        high_dpi: true,
        window_width: 1024,
        window_height: 768,
        ..Default::default()
    }
}

#[macroquad::main(window_conf)]
async fn main() {
    /// Приватная функция
    fn current_day() -> Day {
        use std::time::SystemTime;
        Day(SystemTime::now()
            .duration_since(SystemTime::UNIX_EPOCH)
            .unwrap()
            .as_secs()
            / 60
            / 60
            / 24)
    }
    let today = current_day();

    #[cfg(not(target_arch = "wasm32"))]
    color_backtrace::install();

    let (words, settings, stats) = gui::Program::load();

    let mut pause_detector = PauseDetector::new(
        stats
            .by_day
            .get(&today)
            .map(|x| x.working_time)
            .unwrap_or(0.),
    );

    let mut program = gui::Program::new(
        words,
        settings,
        stats,
        today,
        pause_detector.get_working_time(),
    );

    let texture = Texture2D::from_rgba8(1, 1, &[192, 192, 192, 128]);
    let pause = Texture2D::from_file_with_format(include_bytes!("../pause.png"), None);

    loop {
        clear_background(BLACK);

        egui_macroquad::ui(|ctx| {
            program.ui(ctx, today, pause_detector.get_working_time());
        });
        egui_macroquad::draw();

        if pause_detector.is_paused(program.get_settings()) {
            draw_texture_ex(
                texture,
                0.,
                0.,
                WHITE,
                DrawTextureParams {
                    dest_size: Some(Vec2::new(screen_width(), screen_height())),
                    source: None,
                    rotation: 0.,
                    flip_x: false,
                    flip_y: false,
                    pivot: None,
                },
            );
            draw_texture_ex(
                pause,
                screen_width() / 2. - 100.,
                screen_height() / 2. - 100.,
                WHITE,
                DrawTextureParams {
                    dest_size: Some(Vec2::new(200.0, 200.0)),
                    source: None,
                    rotation: 0.,
                    flip_x: false,
                    flip_y: false,
                    pivot: None,
                },
            );
        }

        next_frame().await;
    }
}
