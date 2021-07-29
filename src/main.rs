use macroquad::prelude::*;
use serde::*;
use std::collections::BTreeMap;
use std::collections::BTreeSet;

/// День
#[derive(Serialize, Deserialize, Clone, Copy)]
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
    right: i32,
    wrong: i32,
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

impl Drop for Words {
    fn drop(&mut self) {
        self.save();
    }
}

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
    fn save(&self) {
        std::fs::write("learn_words.data", ron::to_string(self).unwrap()).unwrap();
    }

    fn calculate_known_words(&self) -> BTreeSet<String> {
        self.0.iter().map(|(word, _)| word.clone()).collect()
    }

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
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect::<Vec<_>>()
}

struct Settings {
    type_count: Vec<LearnType>,
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
        }
    }
}

fn read_clipboard() -> Option<String> {
    miniquad::clipboard::get(unsafe { get_internal_gl().quad_context })
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
        settings: Settings,
    }

    impl Program {
        pub fn new(words: Words, today: Day) -> Self {
            let learn_window = LearnWordsWindow::new(&words, today);
            let known_words = words.calculate_known_words();

            Self {
                data: words,
                known_words,
                learn_window,
                load_text_window: None,
                add_words_window: None,
                add_custom_words_window: None,
                settings: Settings::default(),
            }
        }

        pub fn ui(&mut self, ctx: &CtxRef, today: Day) {
            TopBottomPanel::top("my top").show(ctx, |ui| {
                menu::bar(ui, |ui| {
                    menu::menu(ui, "Add words", |ui| {
                        if ui.button("From text").clicked() {
                            self.load_text_window = Some(LoadTextWindow::new(false));
                        }
                        if ui.button("From subtitles").clicked() {
                            self.load_text_window = Some(LoadTextWindow::new(true));
                        }
                        if ui.button("Manually").clicked() {
                            self.add_custom_words_window = Some(Default::default());
                        }
                    });
                    if ui.button("Save").clicked() {
                        self.data.save();
                    }
                    if ui.button("Debug").clicked() {
                        println!("------------------------------");
                        println!("------------------------------");
                        println!("------------------------------");
                        dbg!(&self.data);
                    }
                    if ui.button("About").clicked() {}
                });
            });

            self.learn_window.ui(ctx, &mut self.data, today);
            if let Some(window) = &mut self.load_text_window {
                use LoadTextAction::*;
                match window.ui(ctx, &self.known_words) {
                    Some(CloseSelf) => self.load_text_window = None,
                    Some(CreateAddWordWindow(words)) => {
                        self.load_text_window = None;
                        self.add_words_window = Some(AddWordsWindow::new(words));
                    }
                    None => {}
                }
            }
            if let Some(window) = &mut self.add_words_window {
                use AddWordsAction::*;
                match window.ui(ctx) {
                    Some(CloseSelf) => {
                        self.add_words_window = None;
                        self.learn_window.update(&self.data, today);
                    }
                    Some(AddWord(word, to_add)) => {
                        self.data.add_word(word, to_add, &self.settings, today);
                    }
                    None => {}
                }
            }
            if let Some(window) = &mut self.add_custom_words_window {
                use AddWordsAction::*;
                match window.ui(ctx) {
                    Some(CloseSelf) => {
                        self.add_custom_words_window = None;
                        self.learn_window.update(&self.data, today);
                    }
                    Some(AddWord(word, to_add)) => {
                        self.data.add_word(word, to_add, &self.settings, today);
                    }
                    None => {}
                }
            }
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
            Self {
                load_subtitles,
                text: read_clipboard().ok_or_else(String::new),
            }
        }

        fn ui(&mut self, ctx: &CtxRef, known_words: &BTreeSet<String>) -> Option<LoadTextAction> {
            let mut opened = true;

            let mut action = None;

            egui::Window::new(if self.load_subtitles {
                "Load words from subtitles"
            } else {
                "Load words from text"
            })
            .open(&mut opened)
            .scroll(false)
            .show(ctx, |ui| {
                ui.horizontal(|ui| {
                    if ui.button("Update clipboard").clicked() {
                        self.text = read_clipboard().ok_or_else(String::new);
                    }
                    if ui.button("Use this text").clicked() {
                        let text = self.text.as_ref().unwrap_or_else(|x| x);
                        let words = if self.load_subtitles {
                            get_words_subtitles(&text)
                        } else {
                            get_words(&text)
                        };
                        let words = words
                            .into_iter()
                            .filter(|x| !known_words.contains(x))
                            .collect();
                        action = Some(LoadTextAction::CreateAddWordWindow(words));
                    }
                });
                match &mut self.text {
                    Ok(text) => {
                        if text.len() > 50 {
                            ui.label(format!(
                                "{}... {:.1} KB",
                                text.chars().take(50).collect::<String>(),
                                text.len() as f64 / 1024.0
                            ));
                        } else {
                            ui.label(&*text);
                        }
                    }
                    Err(text) => {
                        ui.text_edit_multiline(text);
                    }
                }
            });

            if !opened {
                action = Some(LoadTextAction::CloseSelf);
            }

            action
        }
    }

    struct AddWordsWindow {
        words: Vec<String>,
        translations: String,
    }

    enum AddWordsAction {
        CloseSelf,
        AddWord(String, WordsToAdd),
    }

    impl AddWordsWindow {
        fn new(words: Vec<String>) -> Self {
            AddWordsWindow {
                words,
                translations: String::new(),
            }
        }

        fn ui(&mut self, ctx: &CtxRef) -> Option<AddWordsAction> {
            let mut opened = true;

            let mut action = None;

            if !self.words.is_empty() {
                egui::Window::new("Add words")
                    .open(&mut opened)
                    .scroll(false)
                    .show(ctx, |ui| {
                        ui.label(format!("Words remains: {}", self.words.len()));
                        ui.separator();
                        if let Some((word, to_add)) =
                            word_to_add(ui, &mut self.words[0], &mut self.translations)
                        {
                            self.translations.clear();
                            self.words.remove(0);
                            action = Some(AddWordsAction::AddWord(word, to_add));
                        }
                    });
            } else {
                action = Some(AddWordsAction::CloseSelf);
            }

            if !opened {
                action = Some(AddWordsAction::CloseSelf);
            }

            action
        }
    }

    #[derive(Default)]
    struct AddCustomWordsWindow {
        word: String,
        translations: String,
    }

    impl AddCustomWordsWindow {
        fn ui(&mut self, ctx: &CtxRef) -> Option<AddWordsAction> {
            let mut opened = true;

            let mut action = None;

            egui::Window::new("Add words")
                .open(&mut opened)
                .scroll(false)
                .show(ctx, |ui| {
                    ui.separator();
                    if let Some((word, to_add)) =
                        word_to_add(ui, &mut self.word, &mut self.translations)
                    {
                        self.translations.clear();
                        self.word.clear();
                        action = Some(AddWordsAction::AddWord(word, to_add));
                    }
                });

            if !opened {
                action = Some(AddWordsAction::CloseSelf);
            }

            action
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
        },
        Checked {
            word: String,
            known_words: Vec<String>,
            words_to_type: Vec<Result<String, (String, String)>>,
            words_to_guess: Vec<Result<String, (String, String)>>,
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

        fn ui(&mut self, ctx: &CtxRef, words: &mut Words, today: Day) {
            egui::Window::new("Learn words").show(ctx, |ui| match &mut self.current {
                LearnWords::None => {
                    ui.label("🎉🎉🎉 Everything is learned for today! 🎉🎉🎉");
                }
                LearnWords::Typing {
                    word,
                    correct_answer,
                    words_to_type,
                    words_to_guess,
                } => {
                    ui.label(format!("Words remains: {}", self.to_type_today.len()));
                    ui.label(format!("Word: {}", word));

                    for i in &mut correct_answer.known_words {
                        ui.add(egui::TextEdit::singleline(i).enabled(false));
                    }
                    for (hint, i) in correct_answer
                        .words_to_type
                        .iter()
                        .zip(words_to_type.iter_mut())
                    {
                        ui.add(egui::TextEdit::singleline(i).hint_text(hint));
                    }
                    for i in &mut *words_to_guess {
                        ui.add(egui::TextEdit::singleline(i));
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
                            words.register_attempt(word, answer, correct, today);
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
                                words.register_attempt(word, &corrects[position], true, today);
                                corrects.remove(position);
                                words_to_type_result.push(Ok(typed.clone()));
                            } else {
                                let answer = answers.remove(0);
                                words.register_attempt(word, &answer, false, today);
                                words_to_guess_result.push(Err((answer, typed.clone())));
                            }
                        }

                        self.current = LearnWords::Checked {
                            word: word.clone(),
                            known_words: correct_answer.known_words.clone(),
                            words_to_type: words_to_type_result,
                            words_to_guess: words_to_guess_result,
                        };
                    }
                }
                LearnWords::Checked {
                    word,
                    known_words,
                    words_to_type,
                    words_to_guess,
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

                    if ui.button("Next").clicked() {
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
            program.ui(ctx, today);
        });
        egui_macroquad::draw();

        next_frame().await;
    }
}
