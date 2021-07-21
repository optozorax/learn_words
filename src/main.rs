/// День
struct Day(i32);

/// Итерация изучения слова, сколько ждать с последнего изучения, сколько раз повторить, показывать ли слово во время набора
struct LearnType {
    /// Сколько дней ждать с 
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

    /// Мы изучаем это слово
    ToLearn { 
        /// Когда это слово в последний раз изучали
        last_learn: Day, 

        /// Количество изучений слова
        learns: Vec<LearnType>, 

        /// Статистика
        stats: TypingStats, 
    },

    /// Мы знаем это слово
    Learned { 
        /// Статистика
        stats: TypingStats, 
    },
}

fn main() {
    println!("Hello, world!");
}
