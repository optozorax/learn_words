#![allow(dead_code, unused_variables)]

use std::collections::BTreeSet;
use std::collections::BTreeMap;

/// День
struct Day(i32);

/// Итерация изучения слова, сколько ждать с последнего изучения, сколько раз повторить, показывать ли слово во время набора
struct LearnType {
    /// Сколько дней ждать с последнего изучения
    wait_days: i8,
    count: i8,
    show_word: bool,
}

/// Статистика написаний для слова, дня или вообще
struct TypingStats {
    typed: i32,
    right: i32,
    wrong: i32,
}

/// Обозначает одну пару слов рус-англ или англ-рус в статистике
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
    fn register_attempt(&mut self, correct: bool) -> bool {
        todo!()
    }
}

/// Все слова в программе, это используется для хранения информации
struct WordsSaved(BTreeMap<String, WordStatus>);

/// Все слова в программе, это используется для вычислений
struct WordsOptimized {
    all: BTreeMap<String, Vec<WordStatus>>,

    /// Известные, мусорные, выученные, добавленные слова, необходимо для фильтрации после добавления слова
    not_show: BTreeSet<String>,
}

enum WordsToAdd {
    KnowPreviously,
    TrashWord,
    ToLearn {
        translations: Vec<String>,
    },
}

struct WordsToLearn {
    word: String,
    known_words: Vec<String>,
    words_to_type: Vec<String>,
    words_to_guess: Vec<String>,
}

impl WordsOptimized {
    fn new(saved: WordsSaved) -> Self {
        // to_type_today shuffle'ится после создания
        todo!()
    }

    fn save(&self) -> WordsSaved {
        //self.all.clone()
        todo!()
    }

    fn is_unknown_word(&self, word: &str) -> bool {
        todo!()
    }

    fn add_word(&mut self, word: String, info: WordsToAdd, settings: &Settings) {
        // Слово добавляется не только word->translations, а ещё 
        todo!()
    }

    fn get_word_to_learn(&mut self, word: &str) -> WordsToLearn {
        todo!()
    }

    fn register_attempt(&mut self, word: &str, translation: &str, correct: bool) {
        // Если неверно, то надо снова это добавить to_type_today в случайное место
        todo!()
    }
}

fn get_words_subtitles(subtitles: &str) -> Vec<String> {
    todo!()
}

fn get_words(text: &str) -> Vec<String> {
    todo!()
}

fn current_day() -> Day {
    todo!()
}

struct Settings {
    type_count: Vec<LearnType>,
}

impl Default for Settings {
    fn default() -> Self {
        // Все вот эти штуки что в первый раз 2 рааз, потом 3 раза итд
        todo!()
    }
}

mod gui {
    use egui::*;
    use super::*;

    struct Program {
        learn_window: LearnWordsWindow,

    }

    struct LearnWordsWindow {
        /// То что надо ввести несколько раз повторяется, слово повторяется максимальное число из всех под-слов что с ним связано. Если слово уже известно, то надо 
        to_type_today: Vec<String>,
    }

    fn word_to_add(ui: &mut Ui, word: &mut String, translations: &mut String) -> Option<WordsToAdd> {
        // Здесь можно ввести как переводы слова, так и есть кнопки для мусорных и известных слов итд слов
        // После нажатия одной кнопки входные строки очищаются
        todo!()
    }
}

fn main() {
    
}
