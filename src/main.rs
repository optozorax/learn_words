#![allow(dead_code, unused_variables)]

use macroquad::prelude::*;
use serde::*;
use std::collections::BTreeMap;
use std::collections::BTreeSet;

/// День
#[derive(Serialize, Deserialize, Clone, Copy)]
pub struct Day(u64);

/// Итерация изучения слова, сколько ждать с последнего изучения, сколько раз повторить, показывать ли слово во время набора
#[derive(Serialize, Deserialize, Clone)]
struct LearnType {
    /// Сколько дней ждать с последнего изучения
    wait_days: u8,
    count: u8,
    show_word: bool,
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
#[derive(Default, Serialize, Deserialize, Clone, Copy)]
struct TypingStats {
    right: i32,
    wrong: i32,
}

/// Обозначает одну пару слов рус-англ или англ-рус в статистике
#[derive(Serialize, Deserialize, Clone)]
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
    fn register_attempt(&mut self, correct: bool, today: Day) {
        use WordStatus::*;
        match self {
            KnowPreviously | TrashWord | Learned { .. } => unreachable!(),
            ToLearn {
                stats,
                learns,
                last_learn,
                translation,
            } => {
                if correct {
                    stats.right += 1;
                } else {
                    stats.wrong += 1;
                }

                let mut registered = false;

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

    fn has_translation(&self, translation2: &str) -> bool {
        use WordStatus::*;
        match self {
            KnowPreviously | TrashWord => false,
            ToLearn { translation, .. } | Learned { translation, .. } => {
                translation == translation2
            }
        }
    }
}

/// Все слова в программе
#[derive(Default, Serialize, Deserialize, Clone)]
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
    max_attempts: u8,
}

impl Words {
    fn add_word(&mut self, word: String, info: WordsToAdd, settings: &Settings, today: Day) {
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
                        stats: Default::default(),
                    });
                }
                for translation in translations {
                    self.0
                        .entry(translation)
                        .or_insert_with(Vec::new)
                        .push(WordStatus::ToLearn {
                            translation: word.clone(),
                            last_learn: today,
                            learns: settings.type_count.clone(),
                            stats: Default::default(),
                        });
                }
            }
        }
    }

    fn get_word_to_learn(&mut self, word: &str, today: Day) -> WordsToLearn {
        let mut known_words = Vec::new();
        let mut words_to_type = Vec::new();
        let mut words_to_guess = Vec::new();
        let mut max_attempts = 0;
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
                        max_attempts = std::cmp::max(max_attempts, learn.count);
                        if learn.show_word {
                            words_to_type.push(translation.clone());
                        } else {
                            words_to_guess.push(translation.clone());
                        }
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
            max_attempts,
        }
    }

    fn register_attempt(&mut self, word: &str, translation: &str, correct: bool, today: Day) {
        for i in self.0.get_mut(word).unwrap() {
            if i.has_translation(translation) {
                i.register_attempt(correct, today);
                return;
            }
        }
        unreachable!()
    }
}

fn get_words_subtitles(subtitles: &str) -> Vec<String> {
    let subtitles = srtparse::from_str(subtitles).unwrap();
    let text = subtitles
        .into_iter()
        .map(|x| x.text)
        .collect::<Vec<_>>()
        .join("\n");

    get_words(&text)
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
        .collect::<Vec<_>>()
}

struct Settings {
    type_count: Vec<LearnType>,
}

impl Default for Settings {
    fn default() -> Self {
        Settings {
            type_count: vec![
                LearnType::show(0, 2),
                LearnType::guess(0, 3),
                LearnType::guess(2, 5),
                LearnType::guess(7, 5),
                LearnType::guess(20, 5),
            ],
        }
    }
}

mod gui {
    use super::*;
    use egui::*;

    pub struct Program {
        data: Words,
        /// Известные, мусорные, выученные, добавленные слова, необходимо для фильтрации после добавления слова
        known_words: BTreeSet<String>,
        learn_window: LearnWordsWindow,
        load_text_window: Option<LoadTextWindow>,
        add_words_window: Option<AddWordsWindow>,
        add_custom_words_window: Option<AddCustomWordsWindow>,
    }

    pub enum ProgramAction {
        Save,
    }

    impl Program {
        pub fn new(words: Words, today: Day) -> Self {
            // так же вычисляет все слова что сегодня надо изучить

            let learn_window = LearnWordsWindow::new(&words, today);

            Self {
                data: words,
                known_words: Default::default(), // todo
                learn_window,
                load_text_window: None,
                add_words_window: None,
                add_custom_words_window: None,
            }
        }

        pub fn ui(&mut self, ctx: &CtxRef) -> Option<ProgramAction> {
            TopBottomPanel::top("my top").show(ctx, |ui| {
                menu::bar(ui, |ui| {
                    menu::menu(ui, "Add words", |ui| {
                        if ui.button("From text").clicked() {}
                        if ui.button("From subtitles").clicked() {}
                        if ui.button("Manually").clicked() {}
                    });
                    if ui.button("About").clicked() {}
                });
            });

            // todo

            None
        }
    }

    struct LoadTextWindow {
        load_subtitles: bool,
        text: Result<String, String>,
    }

    enum LoadTextAction {
        CloseSelf,
        CreateAddWordWindow(Vec<String>),
    }

    impl LoadTextWindow {
        fn new(load_subtitles: bool) -> Self {
            // Считать текст из буфера обмена сразу, если получилось вернуть Ok(...), иначе Err(...), на второй вариант пользователь должен сам ввести текст или вставить его из буфера обмена
            todo!()
        }

        fn ui(&mut self, ctx: &CtxRef, known_words: &BTreeSet<String>) -> Option<LoadTextAction> {
            // Есть кнопка "обновить информацию из буфера обмена"
            todo!()
        }
    }

    struct AddWordsWindow {
        words: Vec<String>,
        translations: String,
    }

    enum AddWordsAction {
        CloseSelf,
        AddWord(WordsToAdd),
    }

    impl AddWordsWindow {
        fn new(words: Vec<String>) -> Self {
            todo!()
        }

        fn ui(&mut self, ctx: &CtxRef) -> Option<AddWordsAction> {
            todo!()
        }
    }

    struct AddCustomWordsWindow {
        word: String,
        translations: String,
    }

    impl AddCustomWordsWindow {
        fn new(words: Vec<String>) -> Self {
            todo!()
        }

        fn ui(&mut self, ctx: &CtxRef, known_words: &BTreeSet<String>) -> Option<AddWordsAction> {
            todo!()
        }
    }

    // Это окно нельзя закрыть
    struct LearnWordsWindow {
        /// То что надо ввести несколько раз повторяется, слово повторяется максимальное число из всех под-слов что с ним связано. Если слово уже известно, то надо
        to_type_today: Vec<String>,
        current_type: Option<WordsToLearn>,
    }

    impl LearnWordsWindow {
        fn new(words: &Words, today: Day) -> Self {
            // to_type_today shuffle'ится после создания
            //todo!()

            Self {
                to_type_today: vec![],
                current_type: None,
            }
        }

        fn ui(&mut self, ctx: &CtxRef, words: &mut Words) {
            // Если неверно, то надо снова это добавить to_type_today в случайное место
            todo!()
        }
    }

    fn word_to_add(
        ui: &mut Ui,
        word: &mut String,
        translations: &mut String,
    ) -> Option<WordsToAdd> {
        // Здесь можно ввести как переводы слова, так и есть кнопки для мусорных и известных слов итд слов
        // После нажатия одной кнопки входные строки очищаются
        todo!()
    }
}

fn window_conf() -> Conf {
    Conf {
        window_title: "Learn Words".to_owned(),
        high_dpi: true,
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

    let words: Words = std::fs::read_to_string("learn_words.data")
        .map(|x| ron::from_str::<Words>(&x).unwrap())
        .unwrap_or_default();

    let mut program = gui::Program::new(words, today);

    loop {
        clear_background(BLACK);

        egui_macroquad::ui(|ctx| {
            if let Some(action) = program.ui(ctx) {
                // todo
            }
        });
        egui_macroquad::draw();

        next_frame().await;
    }
}
