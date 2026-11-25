//! An invented dictionary used only by the tests.
//!
//! Nothing here comes from a published dictionary. The corpus exists to
//! exercise the Markdown shapes the loader
//! accepts: `\*` entry boundaries, alias lines, cross-file links, an index that
//! defines the anchors, the four appendix formats, escaped brackets, and a
//! redirect. It is written to a temporary directory at test time, so no
//! dictionary-shaped data is stored in the repository.

use std::fs;
use std::path::Path;

pub const FILES: [(&str, &str); 8] = [
    ("08-a.md", LETTER_A),
    ("10-c.md", LETTER_C),
    ("14-g.md", LETTER_G),
    ("34-appendix-1-Meridian.md", MERIDIAN),
    ("35-appendix-2-Medicinal.md", MEDICINAL),
    ("36-appendix-3-Formulas.md", FORMULAS),
    ("37-appendix-4-Weights-Measurements.md", WEIGHTS),
    ("38-index.md", INDEX),
];

const LETTER_A: &str = r#"<a href="03-table-of-contents.md#a1TOC" class="class_s1P94">A</a>

\*alpha

甲 | 甲  \*jia3 / jiǎ

Also alpha alias. See <a href="14-g.md#a1BET">greater alpha</a>.

\*alpha swelling

甲肿 | 甲腫  \*jia3 zhong3 / jiǎ zhǒng
甲胀 | 甲脹  \*jia3 zhang4 / jiǎ zhàng

A made-up feeling of swelling in the alpha region. See <a href="10-c.md#a1CHT">clear heat</a>.

\*alpha alias

甲别 | 甲別  \*jia3 bie2 / jiǎ bié

See <a href="08-a.md#a1ALP">alpha</a>.

\*beta

乙 | 乙  \*yi3 / yǐ

A made-up second entry that keeps the main dictionary at six entries.
"#;

const LETTER_C: &str = r#"\*clear heat

清甲 | 清甲  \*qing1 jia3 / qīng jiǎ

A made-up treatment method that clears heat.
"#;

const LETTER_G: &str = r#"\*greater alpha

大甲 | 大甲  \*da4 jia3 / dà jiǎ

The upper part of the alpha region.
"#;

const MERIDIAN: &str = r#"<a href="03-table-of-contents.md#a1TOC" class="class_s1PMH">Appendix 1</a>

1.3 AA, Made-up Channel

AA-1 (甲三里 / 甲三里 jiǎ sān lǐ / jia san li, Jia Three Li): Alt. Names: 下甲 xià jiǎ, Lower Jia.

• Location: On the made-up limb.

• Actions: Fortifies the liver.
"#;

const MEDICINAL: &str = r#"<a href="03-table-of-contents.md#a1TOC" class="class_s1PMH">Appendix 2</a>

2.1 Made-up Agents

\[01.01.01\] JIǍ GĒN / jia gen 甲根 / 甲根 (Jiae Herba, jia root): Alt. Eng.: jiagen. Category: Made-up agent.

• Properties: Bland, warm.
"#;

const FORMULAS: &str = r#"<a href="03-table-of-contents.md#a1TOC" class="class_s1PMH">Appendix 3</a>

\[01.01.01\] JIǍ GĒN TĀNG / jia gen tang 甲根汤 / 甲根湯 (Jia Decoction): Made-up warm formula.
"#;

const WEIGHTS: &str = r#"<a href="03-table-of-contents.md#a1TOC" class="class_s1PMH">Appendix 4: Weights and Measures</a>

21\. Weights in Different Dynasties

| Dynasty | Grams/jin |
|---------|-----------|
| Zhou    | 14.18     |

22\. Measures in Different Dynasties

| Dynasty | Ml./dou |
|---------|---------|
| Zhou    | 193.7   |

23\. Weights Conversion Table

| Unit | Value |
|------|-------|
| jin  | 1     |
"#;

const INDEX: &str = r#"Index

<a href="08-a.md#a1ALP" class="class_s1NTM-0">alpha</a>

<a href="08-a.md#a1ASW" class="class_s1NTM-0">alpha swelling</a>

<a href="08-a.md#a1AAL" class="class_s1NTM-0">alpha alias</a>

<a href="08-a.md#a1BTA" class="class_s1NTM-0">beta</a>

<a href="14-g.md#a1BET" class="class_s1NTM-0">greater alpha</a>

<a href="10-c.md#a1CHT" class="class_s1NTM-0">clear heat</a>
"#;

/// Write the invented source dictionary into `destination`.
pub fn write(destination: &Path) {
    fs::create_dir_all(destination).expect("the synthetic fixture directory is created");
    for (name, text) in FILES {
        fs::write(destination.join(name), text).expect("a synthetic fixture file is written");
    }
}
