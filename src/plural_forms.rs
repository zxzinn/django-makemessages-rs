//! Plural-Forms headers for each locale, mirroring what Django's
//! `copy_plural_forms` copies out of its own shipped catalogs in
//! `django/conf/locale/<locale>/LC_MESSAGES/django.po`.
//!
//! Generated from Django 6.0.7. Regenerate with
//! `tests/tools/gen_plural_forms.py` when syncing to a newer Django.

/// Default used when a locale is not in Django's catalog set; this is
/// also what gettext falls back to.
pub const DEFAULT_PLURAL_FORMS: &str = "nplurals=2; plural=(n != 1);";

/// (locale, Plural-Forms value) sorted by locale for binary search.
pub static PLURAL_FORMS: [(&str, &str); 98] = [
    ("af", "nplurals=2; plural=(n != 1);"),
    ("ar", "nplurals=6; plural=n==0 ? 0 : n==1 ? 1 : n==2 ? 2 : n%100>=3 && n%100<=10 ? 3 : n%100>=11 && n%100<=99 ? 4 : 5;"),
    ("ar_DZ", "nplurals=6; plural=n==0 ? 0 : n==1 ? 1 : n==2 ? 2 : n%100>=3 && n%100<=10 ? 3 : n%100>=11 && n%100<=99 ? 4 : 5;"),
    ("ast", "nplurals=2; plural=(n != 1);"),
    ("az", "nplurals=2; plural=(n != 1);"),
    ("be", "nplurals=4; plural=(n%10==1 && n%100!=11 ? 0 : n%10>=2 && n%10<=4 && (n%100<12 || n%100>14) ? 1 : n%10==0 || (n%10>=5 && n%10<=9) || (n%100>=11 && n%100<=14)? 2 : 3);"),
    ("bg", "nplurals=2; plural=(n != 1);"),
    ("bn", "nplurals=2; plural=(n != 1);"),
    ("br", "nplurals=5; plural=((n%10 == 1) && (n%100 != 11) && (n%100 !=71) && (n%100 !=91) ? 0 :(n%10 == 2) && (n%100 != 12) && (n%100 !=72) && (n%100 !=92) ? 1 :(n%10 ==3 || n%10==4 || n%10==9) && (n%100 < 10 || n% 100 > 19) && (n%100 < 70 || n%100 > 79) && (n%100 < 90 || n%100 > 99) ? 2 :(n != 0 && n % 1000000 == 0) ? 3 : 4);"),
    ("bs", "nplurals=3; plural=(n%10==1 && n%100!=11 ? 0 : n%10>=2 && n%10<=4 && (n%100<10 || n%100>=20) ? 1 : 2);"),
    ("ca", "nplurals=2; plural=(n != 1);"),
    ("ckb", "nplurals=2; plural=(n != 1);"),
    ("cs", "nplurals=4; plural=(n == 1 && n % 1 == 0) ? 0 : (n >= 2 && n <= 4 && n % 1 == 0) ? 1: (n % 1 != 0 ) ? 2 : 3;"),
    ("cy", "nplurals=4; plural=(n==1) ? 0 : (n==2) ? 1 : (n != 8 && n != 11) ? 2 : 3;"),
    ("da", "nplurals=2; plural=(n != 1);"),
    ("de", "nplurals=2; plural=(n != 1);"),
    ("dsb", "nplurals=4; plural=(n%100==1 ? 0 : n%100==2 ? 1 : n%100==3 || n%100==4 ? 2 : 3);"),
    ("el", "nplurals=2; plural=(n != 1);"),
    ("en", "nplurals=2; plural=(n != 1);"),
    ("en_AU", "nplurals=2; plural=(n != 1);"),
    ("en_GB", "nplurals=2; plural=(n != 1);"),
    ("eo", "nplurals=2; plural=(n != 1);"),
    ("es", "nplurals=2; plural=(n != 1);"),
    ("es_AR", "nplurals=2; plural=(n != 1);"),
    ("es_CO", "nplurals=2; plural=(n != 1);"),
    ("es_MX", "nplurals=2; plural=(n != 1);"),
    ("es_VE", "nplurals=2; plural=(n != 1);"),
    ("et", "nplurals=2; plural=(n != 1);"),
    ("eu", "nplurals=2; plural=(n != 1);"),
    ("fa", "nplurals=2; plural=(n > 1);"),
    ("fi", "nplurals=2; plural=(n != 1);"),
    ("fr", "nplurals=2; plural=(n > 1);"),
    ("fy", "nplurals=2; plural=(n != 1);"),
    ("ga", "nplurals=5; plural=(n==1 ? 0 : n==2 ? 1 : n<7 ? 2 : n<11 ? 3 : 4);"),
    ("gd", "nplurals=4; plural=(n==1 || n==11) ? 0 : (n==2 || n==12) ? 1 : (n > 2 && n < 20) ? 2 : 3;"),
    ("gl", "nplurals=2; plural=(n != 1);"),
    ("he", "nplurals=3; plural=(n == 1 && n % 1 == 0) ? 0 : (n == 2 && n % 1 == 0) ? 1: 2;"),
    ("hi", "nplurals=2; plural=(n != 1);"),
    ("hr", "nplurals=3; plural=n%10==1 && n%100!=11 ? 0 : n%10>=2 && n%10<=4 && (n%100<10 || n%100>=20) ? 1 : 2;"),
    ("hsb", "nplurals=4; plural=(n%100==1 ? 0 : n%100==2 ? 1 : n%100==3 || n%100==4 ? 2 : 3);"),
    ("hu", "nplurals=2; plural=(n != 1);"),
    ("hy", "nplurals=2; plural=(n != 1);"),
    ("ia", "nplurals=2; plural=(n != 1);"),
    ("id", "nplurals=1; plural=0;"),
    ("ig", "nplurals=1; plural=0;"),
    ("io", "nplurals=2; plural=(n != 1);"),
    ("is", "nplurals=2; plural=(n % 10 != 1 || n % 100 == 11);"),
    ("it", "nplurals=2; plural=(n != 1);"),
    ("ja", "nplurals=1; plural=0;"),
    ("ka", "nplurals=2; plural=(n!=1);"),
    ("kab", "nplurals=2; plural=(n != 1);"),
    ("kk", "nplurals=2; plural=(n!=1);"),
    ("km", "nplurals=1; plural=0;"),
    ("kn", "nplurals=2; plural=(n > 1);"),
    ("ko", "nplurals=1; plural=0;"),
    ("ky", "nplurals=1; plural=0;"),
    ("lb", "nplurals=2; plural=(n != 1);"),
    ("lt", "nplurals=4; plural=(n % 10 == 1 && (n % 100 > 19 || n % 100 < 11) ? 0 : (n % 10 >= 2 && n % 10 <=9) && (n % 100 > 19 || n % 100 < 11) ? 1 : n % 1 != 0 ? 2: 3);"),
    ("lv", "nplurals=3; plural=(n%10==1 && n%100!=11 ? 0 : n != 0 ? 1 : 2);"),
    ("mk", "nplurals=2; plural=(n % 10 == 1 && n % 100 != 11) ? 0 : 1;"),
    ("ml", "nplurals=2; plural=(n != 1);"),
    ("mn", "nplurals=2; plural=(n != 1);"),
    ("mr", "nplurals=2; plural=(n != 1);"),
    ("ms", "nplurals=1; plural=0;"),
    ("my", "nplurals=1; plural=0;"),
    ("nb", "nplurals=2; plural=(n != 1);"),
    ("ne", "nplurals=2; plural=(n != 1);"),
    ("nl", "nplurals=2; plural=(n != 1);"),
    ("nn", "nplurals=2; plural=(n != 1);"),
    ("os", "nplurals=2; plural=(n != 1);"),
    ("pa", "nplurals=2; plural=(n != 1);"),
    ("pl", "nplurals=4; plural=(n==1 ? 0 : (n%10>=2 && n%10<=4) && (n%100<12 || n%100>14) ? 1 : n!=1 && (n%10>=0 && n%10<=1) || (n%10>=5 && n%10<=9) || (n%100>=12 && n%100<=14) ? 2 : 3);"),
    ("pt", "nplurals=2; plural=(n != 1);"),
    ("pt_BR", "nplurals=2; plural=(n > 1);"),
    ("ro", "nplurals=3; plural=(n==1?0:(((n%100>19)||((n%100==0)&&(n!=0)))?2:1));"),
    ("ru", "nplurals=4; plural=(n%10==1 && n%100!=11 ? 0 : n%10>=2 && n%10<=4 && (n%100<12 || n%100>14) ? 1 : n%10==0 || (n%10>=5 && n%10<=9) || (n%100>=11 && n%100<=14)? 2 : 3);"),
    ("sk", "nplurals=4; plural=(n % 1 == 0 && n == 1 ? 0 : n % 1 == 0 && n >= 2 && n <= 4 ? 1 : n % 1 != 0 ? 2: 3);"),
    ("sl", "nplurals=4; plural=(n%100==1 ? 0 : n%100==2 ? 1 : n%100==3 || n%100==4 ? 2 : 3);"),
    ("sq", "nplurals=2; plural=(n != 1);"),
    ("sr", "nplurals=3; plural=(n%10==1 && n%100!=11 ? 0 : n%10>=2 && n%10<=4 && (n%100<10 || n%100>=20) ? 1 : 2);"),
    ("sr_Latn", "nplurals=3; plural=(n%10==1 && n%100!=11 ? 0 : n%10>=2 && n%10<=4 && (n%100<10 || n%100>=20) ? 1 : 2);"),
    ("sv", "nplurals=2; plural=(n != 1);"),
    ("sw", "nplurals=2; plural=(n != 1);"),
    ("ta", "nplurals=2; plural=(n != 1);"),
    ("te", "nplurals=2; plural=(n != 1);"),
    ("tg", "nplurals=2; plural=(n != 1);"),
    ("th", "nplurals=1; plural=0;"),
    ("tk", "nplurals=2; plural=(n != 1);"),
    ("tr", "nplurals=2; plural=(n > 1);"),
    ("tt", "nplurals=1; plural=0;"),
    ("udm", "nplurals=1; plural=0;"),
    ("ug", "nplurals=2; plural=(n != 1);"),
    ("uk", "nplurals=4; plural=(n % 1 == 0 && n % 10 == 1 && n % 100 != 11 ? 0 : n % 1 == 0 && n % 10 >= 2 && n % 10 <= 4 && (n % 100 < 12 || n % 100 > 14) ? 1 : n % 1 == 0 && (n % 10 ==0 || (n % 10 >=5 && n % 10 <=9) || (n % 100 >=11 && n % 100 <=14 )) ? 2: 3);"),
    ("ur", "nplurals=2; plural=(n != 1);"),
    ("uz", "nplurals=1; plural=0;"),
    ("vi", "nplurals=1; plural=0;"),
    ("zh_Hans", "nplurals=1; plural=0;"),
    ("zh_Hant", "nplurals=1; plural=0;"),
];

/// Look up a locale's Plural-Forms, falling back to the base language
/// (`pt_XX` -> `pt`) the way gettext catalogs are conventionally organized.
pub fn plural_forms_for(locale: &str) -> &'static str {
    if let Ok(i) = PLURAL_FORMS.binary_search_by(|(k, _)| (*k).cmp(locale)) {
        return PLURAL_FORMS[i].1;
    }
    if let Some((base, _)) = locale.split_once('_') {
        if let Ok(i) = PLURAL_FORMS.binary_search_by(|(k, _)| (*k).cmp(base)) {
            return PLURAL_FORMS[i].1;
        }
    }
    DEFAULT_PLURAL_FORMS
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_table_is_sorted_for_binary_search() {
        let mut sorted = PLURAL_FORMS.to_vec();
        sorted.sort_by(|a, b| a.0.cmp(b.0));
        assert_eq!(sorted, PLURAL_FORMS.to_vec());
    }

    #[test]
    fn test_cjk_locales_have_one_plural() {
        for locale in ["ja", "ko", "zh_Hans", "zh_Hant"] {
            assert_eq!(
                plural_forms_for(locale),
                "nplurals=1; plural=0;",
                "wrong plural forms for {locale}"
            );
        }
    }

    #[test]
    fn test_french_uses_greater_than_one() {
        assert_eq!(plural_forms_for("fr"), "nplurals=2; plural=(n > 1);");
    }

    #[test]
    fn test_falls_back_to_base_language() {
        // pt_PT is not in Django's catalog set; pt is.
        assert_eq!(plural_forms_for("pt_PT"), plural_forms_for("pt"));
    }

    #[test]
    fn test_unknown_locale_uses_default() {
        assert_eq!(plural_forms_for("xx_YY"), DEFAULT_PLURAL_FORMS);
    }
}
