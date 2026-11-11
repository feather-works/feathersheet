//! Moedas para o formato de célula: cada uma vira um código de formato do Excel
//! (`[$CHF-807] #,##0.00`, `#,##0.00 [$€-40C]`), que a planilha já mostra
//! ([`crate::celulas::formato_personalizado`]) e o `.xlsx` grava como está. O
//! separador de milhar e o decimal seguem a língua do app, como no Excel.

/// Uma moeda do catálogo.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Moeda {
    /// Código ISO 4217 (`BRL`, `CHF`).
    pub iso: &'static str,
    /// O que aparece na célula (`R$`, `CHF`, `Rs`).
    pub simbolo: &'static str,
    /// Nome em português; a interface traduz.
    pub nome: &'static str,
    /// Código de região do Excel em hexadecimal (`416` = pt-BR); vazio quando não há.
    pub regiao: &'static str,
    /// O símbolo vai depois do número (`1.234,56 €` na França).
    pub depois: bool,
    /// Casas decimais de costume (iene, won e peso chileno não têm centavos).
    pub casas: u8,
}

const fn m(iso: &'static str, simbolo: &'static str, nome: &'static str, regiao: &'static str, depois: bool, casas: u8) -> Moeda {
    Moeda { iso, simbolo, nome, regiao, depois, casas }
}

/// As mais usadas primeiro (o menu da faixa mostra as primeiras); depois por região.
pub const MOEDAS: &[Moeda] = &[
    m("BRL", "R$", "Real brasileiro", "416", false, 2),
    m("USD", "$", "Dólar americano", "409", false, 2),
    m("EUR", "€", "Euro (França)", "40C", true, 2),
    m("EUR", "€", "Euro (€ antes)", "2", false, 2),
    m("GBP", "£", "Libra esterlina", "809", false, 2),
    m("CHF", "CHF", "Franco suíço", "807", false, 2),
    m("PKR", "Rs", "Rupia paquistanesa", "420", false, 2),
    m("INR", "₹", "Rupia indiana", "4009", false, 2),
    m("JPY", "¥", "Iene japonês", "411", false, 0),
    m("CNY", "¥", "Yuan chinês", "804", false, 2),
    // Américas
    m("CAD", "$", "Dólar canadense", "1009", false, 2),
    m("MXN", "$", "Peso mexicano", "80A", false, 2),
    m("ARS", "$", "Peso argentino", "2C0A", false, 2),
    m("CLP", "$", "Peso chileno", "340A", false, 0),
    m("COP", "$", "Peso colombiano", "240A", false, 2),
    m("PEN", "S/", "Sol peruano", "280A", false, 2),
    m("UYU", "$U", "Peso uruguaio", "380A", false, 2),
    m("PYG", "₲", "Guarani paraguaio", "3C0A", false, 0),
    m("BOB", "Bs", "Boliviano", "400A", false, 2),
    m("VES", "Bs.S", "Bolívar venezuelano", "200A", false, 2),
    m("PAB", "B/.", "Balboa panamenho", "", false, 2),
    m("CRC", "₡", "Colón costarriquenho", "140A", false, 2),
    m("GTQ", "Q", "Quetzal guatemalteco", "100A", false, 2),
    m("HNL", "L", "Lempira hondurenha", "480A", false, 2),
    m("NIO", "C$", "Córdoba nicaraguense", "4C0A", false, 2),
    m("DOP", "RD$", "Peso dominicano", "1C0A", false, 2),
    m("CUP", "CUP", "Peso cubano", "", false, 2),
    m("HTG", "G", "Gourde haitiano", "", false, 2),
    m("JMD", "J$", "Dólar jamaicano", "2009", false, 2),
    m("TTD", "TT$", "Dólar de Trinidad e Tobago", "2C09", false, 2),
    m("BSD", "B$", "Dólar baamense", "", false, 2),
    m("BBD", "Bds$", "Dólar de Barbados", "", false, 2),
    m("BZD", "BZ$", "Dólar de Belize", "2809", false, 2),
    m("BMD", "BD$", "Dólar das Bermudas", "", false, 2),
    m("KYD", "CI$", "Dólar das Ilhas Cayman", "", false, 2),
    m("XCD", "EC$", "Dólar do Caribe Oriental", "", false, 2),
    m("AWG", "Afl.", "Florim arubano", "", false, 2),
    m("XCG", "Cg", "Florim do Caribe", "", false, 2),
    m("SRD", "Sr$", "Dólar surinamês", "", false, 2),
    m("GYD", "G$", "Dólar guianense", "", false, 2),
    m("FKP", "£", "Libra das Malvinas", "", false, 2),
    // Europa
    m("SEK", "kr", "Coroa sueca", "41D", true, 2),
    m("NOK", "kr", "Coroa norueguesa", "414", true, 2),
    m("DKK", "kr.", "Coroa dinamarquesa", "406", true, 2),
    m("PLN", "zł", "Zlóti polonês", "415", true, 2),
    m("CZK", "Kč", "Coroa tcheca", "405", true, 2),
    m("HUF", "Ft", "Florim húngaro", "40E", true, 2),
    m("RON", "lei", "Leu romeno", "418", true, 2),
    m("RUB", "₽", "Rublo russo", "419", true, 2),
    m("UAH", "₴", "Grívnia ucraniana", "422", true, 2),
    m("TRY", "₺", "Lira turca", "41F", false, 2),
    m("ISK", "kr", "Coroa islandesa", "40F", true, 0),
    m("ALL", "Lek", "Lek albanês", "41C", true, 2),
    m("BAM", "KM", "Marco conversível bósnio", "141A", true, 2),
    m("MKD", "MKD", "Dinar macedônio", "42F", true, 2),
    m("RSD", "RSD", "Dinar sérvio", "241A", true, 2),
    m("MDL", "L", "Leu moldávio", "818", true, 2),
    m("BYN", "Br", "Rublo bielorrusso", "423", true, 2),
    m("GIP", "£", "Libra de Gibraltar", "", false, 2),
    // Ásia e Oceania
    m("KRW", "₩", "Won sul-coreano", "412", false, 0),
    m("HKD", "HK$", "Dólar de Hong Kong", "C04", false, 2),
    m("TWD", "NT$", "Dólar taiwanês", "404", false, 2),
    m("SGD", "S$", "Dólar de Singapura", "1004", false, 2),
    m("AUD", "$", "Dólar australiano", "C09", false, 2),
    m("NZD", "$", "Dólar neozelandês", "1409", false, 2),
    m("IDR", "Rp", "Rupia indonésia", "421", false, 0),
    m("MYR", "RM", "Ringgit malaio", "43E", false, 2),
    m("PHP", "₱", "Peso filipino", "3409", false, 2),
    m("THB", "฿", "Baht tailandês", "41E", false, 2),
    m("VND", "₫", "Dongue vietnamita", "42A", true, 0),
    m("BDT", "Tk", "Taka bengali", "845", false, 2),
    m("LKR", "Rs", "Rupia do Sri Lanka", "45B", false, 2),
    m("NPR", "Rs", "Rupia nepalesa", "461", false, 2),
    m("ILS", "₪", "Novo shekel israelense", "40D", false, 2),
    m("AED", "AED", "Dirham dos Emirados", "3801", false, 2),
    m("SAR", "SAR", "Rial saudita", "401", false, 2),
    m("QAR", "QAR", "Rial catariano", "4001", false, 2),
    m("KZT", "₸", "Tenge cazaque", "43F", true, 2),
    m("UZS", "UZS", "Som uzbeque", "443", true, 2),
    m("KGS", "KGS", "Som quirguiz", "440", true, 2),
    m("TJS", "TJS", "Somoni tadjique", "428", true, 2),
    m("TMT", "TMT", "Manat turcomeno", "442", true, 2),
    m("AZN", "₼", "Manat azerbaijano", "42C", true, 2),
    m("GEL", "₾", "Lari georgiano", "437", true, 2),
    m("AMD", "AMD", "Dram armênio", "42B", true, 2),
    m("AFN", "Af", "Afegane afegão", "", false, 2),
    m("IRR", "IRR", "Rial iraniano", "429", false, 0),
    m("IQD", "IQD", "Dinar iraquiano", "801", false, 0),
    m("KWD", "KWD", "Dinar kuwaitiano", "3401", false, 3),
    m("BHD", "BHD", "Dinar bareinita", "3C01", false, 3),
    m("OMR", "OMR", "Rial omanense", "2001", false, 3),
    m("JOD", "JOD", "Dinar jordaniano", "2C01", false, 3),
    m("LBP", "LBP", "Libra libanesa", "3001", false, 0),
    m("SYP", "SYP", "Libra síria", "2801", false, 0),
    m("YER", "YER", "Rial iemenita", "2401", false, 0),
    m("MNT", "₮", "Tugrik mongol", "450", false, 2),
    m("MMK", "K", "Quiate de Mianmar", "", false, 0),
    m("KHR", "KHR", "Riel cambojano", "453", true, 0),
    m("LAK", "₭", "Kip laosiano", "454", false, 0),
    m("BND", "B$", "Dólar de Brunei", "83E", false, 2),
    m("MOP", "MOP$", "Pataca de Macau", "1404", false, 2),
    m("MVR", "Rf", "Rufia maldiva", "465", false, 2),
    m("BTN", "Nu.", "Ngultrum butanês", "", false, 2),
    m("KPW", "₩", "Won norte-coreano", "", false, 0),
    m("PGK", "K", "Kina papuásia", "", false, 2),
    m("FJD", "FJ$", "Dólar fijiano", "", false, 2),
    m("SBD", "SI$", "Dólar das Ilhas Salomão", "", false, 2),
    m("TOP", "T$", "Paanga tonganesa", "", false, 2),
    m("WST", "WS$", "Tala samoano", "", false, 2),
    m("VUV", "VT", "Vatu de Vanuatu", "", false, 0),
    m("XPF", "XPF", "Franco CFP", "", true, 0),
    // África
    m("ZAR", "R", "Rand sul-africano", "1C09", false, 2),
    m("EGP", "E£", "Libra egípcia", "C01", false, 2),
    m("NGN", "₦", "Naira nigeriana", "", false, 2),
    m("KES", "KSh", "Xelim queniano", "", false, 2),
    m("MAD", "MAD", "Dirham marroquino", "1801", false, 2),
    m("AOA", "Kz", "Kwanza angolano", "", true, 2),
    m("MZN", "MT", "Metical moçambicano", "", true, 2),
    m("CVE", "$", "Escudo cabo-verdiano", "", true, 2),
    m("DZD", "DA", "Dinar argelino", "1401", true, 2),
    m("TND", "DT", "Dinar tunisiano", "1C01", true, 3),
    m("LYD", "LD", "Dinar líbio", "1001", false, 3),
    m("SDG", "SDG", "Libra sudanesa", "", false, 2),
    m("SSP", "SSP", "Libra sul-sudanesa", "", false, 2),
    m("ETB", "Br", "Birr etíope", "45E", false, 2),
    m("ERN", "Nfk", "Nakfa eritreia", "", false, 2),
    m("DJF", "Fdj", "Franco do Djibuti", "", false, 0),
    m("SOS", "Sh", "Xelim somali", "", false, 2),
    m("UGX", "USh", "Xelim ugandense", "", false, 0),
    m("TZS", "TSh", "Xelim tanzaniano", "", false, 2),
    m("RWF", "FRw", "Franco ruandês", "487", false, 0),
    m("BIF", "FBu", "Franco burundinês", "", false, 0),
    m("CDF", "FC", "Franco congolês", "", true, 2),
    m("XAF", "FCFA", "Franco CFA da África Central", "", true, 0),
    m("XOF", "CFA", "Franco CFA da África Ocidental", "", true, 0),
    m("GHS", "GH₵", "Cedi ganês", "", false, 2),
    m("GMD", "D", "Dalasi gambiano", "", false, 2),
    m("GNF", "FG", "Franco guineano", "", false, 0),
    m("SLE", "Le", "Leone de Serra Leoa", "", false, 2),
    m("LRD", "L$", "Dólar liberiano", "", false, 2),
    m("MRU", "UM", "Uguia mauritana", "", true, 2),
    m("MUR", "Rs", "Rupia mauriciana", "", false, 2),
    m("SCR", "SR", "Rupia seichelense", "", false, 2),
    m("MGA", "Ar", "Ariary malgaxe", "", false, 0),
    m("KMF", "CF", "Franco comoriano", "", false, 0),
    m("MWK", "MK", "Kwacha malauiana", "", false, 2),
    m("ZMW", "ZK", "Kwacha zambiana", "", false, 2),
    m("ZWG", "ZiG", "Ouro do Zimbábue", "", false, 2),
    m("BWP", "P", "Pula botsuanesa", "", false, 2),
    m("NAD", "N$", "Dólar namibiano", "", false, 2),
    m("LSL", "L", "Loti do Lesoto", "", false, 2),
    m("SZL", "E", "Lilangeni de Essuatíni", "", false, 2),
    m("STN", "Db", "Dobra de São Tomé e Príncipe", "", true, 2),
    m("SHP", "£", "Libra de Santa Helena", "", false, 2),
    // Outras
    m("XBT", "₿", "Bitcoin", "", false, 8),
];

impl Moeda {
    /// A etiqueta do código: `[$CHF-807]`, ou `[$Kz]` sem região.
    fn etiqueta(&self) -> String {
        if self.regiao.is_empty() { format!("[${}]", self.simbolo) } else { format!("[${}-{}]", self.simbolo, self.regiao) }
    }

    /// Agrupa em lakh e crore (`12,34,567`), como se escreve na Índia, no Paquistão,
    /// em Bangladesh e no Nepal.
    pub fn lakh(&self) -> bool {
        matches!(self.iso, "INR" | "PKR" | "BDT" | "NPR")
    }

    /// O código de formato com `casas` decimais: `[$CHF-807] #,##0.00`.
    pub fn codigo(&self, casas: u8) -> String {
        let inteiro = if self.lakh() { "#,##,##0" } else { "#,##0" };
        let numero = if casas == 0 { inteiro.to_owned() } else { format!("{inteiro}.{}", "0".repeat(casas.min(15) as usize)) };
        if self.depois { format!("{numero} {}", self.etiqueta()) } else { format!("{} {numero}", self.etiqueta()) }
    }

    /// O código com as casas de costume da moeda.
    pub fn codigo_padrao(&self) -> String {
        self.codigo(self.casas)
    }
}

/// A moeda de um código de formato, se ele for um dos do catálogo (para marcar a
/// escolhida no menu). Compara a etiqueta, não as casas: `[$CHF-807] #,##0` também é franco.
pub fn do_codigo(codigo: &str) -> Option<&'static Moeda> {
    MOEDAS.iter().find(|m| codigo.contains(&m.etiqueta()))
}

/// As casas decimais de um código do catálogo (`[$CHF-807] #,##0.00` → 2).
pub fn casas_do_codigo(codigo: &str) -> u8 {
    codigo.split_once("#,##0").map_or(0, |(_, resto)| resto.strip_prefix('.')
        .map_or(0, |d| d.chars().take_while(|c| *c == '0').count() as u8))
}

/// As moedas cujo nome (em português ou `traduzido`), código ISO ou símbolo contém
/// `texto`, sem acento e sem caixa: "suico", "chf" e "franco" acham o franco suíço.
pub fn buscar<'a>(texto: &str, traduzido: impl Fn(&'static str) -> &'a str) -> Vec<&'static Moeda> {
    fn simples(t: &str) -> String {
        t.to_lowercase().chars().map(|c| match c {
            'á' | 'à' | 'â' | 'ã' => 'a', 'é' | 'ê' => 'e', 'í' => 'i', 'ó' | 'ô' | 'õ' => 'o', 'ú' | 'ü' => 'u', 'ç' => 'c', c => c,
        }).collect()
    }
    let busca = simples(texto.trim());
    MOEDAS.iter().filter(|m| busca.is_empty()
        || [traduzido(m.nome), m.nome, m.iso, m.simbolo].iter().any(|t| simples(t).contains(&busca))).collect()
}

#[cfg(test)]
mod testes {
    use super::*;
    use crate::celulas::formato_personalizado::formatar;
    use crate::celulas::values::Value;

    fn mostra(iso: &str, nome: &str, n: f64) -> String {
        let m = MOEDAS.iter().find(|m| m.iso == iso && m.nome.contains(nome)).unwrap();
        formatar(&Value::Number(n), &m.codigo_padrao()).unwrap()
    }

    #[test]
    fn codigos_saem_como_o_excel_grava() {
        let chf = MOEDAS.iter().find(|m| m.iso == "CHF").unwrap();
        assert_eq!(chf.codigo(2), "[$CHF-807] #,##0.00");
        let fr = MOEDAS.iter().find(|m| m.nome == "Euro (França)").unwrap();
        assert_eq!(fr.codigo(2), "#,##0.00 [$€-40C]");
        let kz = MOEDAS.iter().find(|m| m.iso == "AOA").unwrap();
        assert_eq!(kz.codigo(0), "#,##0 [$Kz]");
    }

    #[test]
    fn a_celula_mostra_o_simbolo_no_lugar_certo() {
        assert_eq!(mostra("CHF", "", 1234.5), "CHF 1.234,50");
        assert_eq!(mostra("PKR", "", 1234.5), "Rs 1.234,50");
        assert_eq!(mostra("INR", "", 12_345_678.9), "₹ 1.23.45.678,90", "lakh e crore");
        assert_eq!(mostra("EUR", "França", 1234.5), "1.234,50 €");
        assert_eq!(mostra("USD", "", 1234.5), "$ 1.234,50");
        assert_eq!(mostra("JPY", "", 1234.5), "¥ 1.235");
        assert_eq!(mostra("AOA", "", 10.0), "10,00 Kz");
    }

    #[test]
    fn casas_saem_do_codigo() {
        let chf = MOEDAS.iter().find(|m| m.iso == "CHF").unwrap();
        for n in [0, 1, 2, 5] { assert_eq!(casas_do_codigo(&chf.codigo(n)), n); }
        let fr = MOEDAS.iter().find(|m| m.nome == "Euro (França)").unwrap();
        assert_eq!(casas_do_codigo(&fr.codigo(3)), 3);
    }

    #[test]
    fn busca_sem_acento_por_nome_codigo_ou_simbolo() {
        let isos = |t: &str| buscar(t, |n| n).iter().map(|m| m.iso).collect::<Vec<_>>();
        assert_eq!(isos("suico"), ["CHF"]);
        assert_eq!(isos("pkr"), ["PKR"]);
        assert!(isos("rupia").len() >= 6);
        assert_eq!(isos("").len(), MOEDAS.len());
        assert_eq!(buscar("swiss", |n| if n == "Franco suíço" { "Swiss franc" } else { n }).len(), 1);
    }

    #[test]
    fn um_codigo_iso_por_moeda_exceto_as_duas_do_euro() {
        let mut vistos = std::collections::HashSet::new();
        for m in MOEDAS { assert!(vistos.insert(m.iso) || m.iso == "EUR", "{} repetida", m.iso); }
        assert!(MOEDAS.len() >= 150);
    }

    #[test]
    fn todo_codigo_do_catalogo_e_reconhecido_e_formata() {
        for m in MOEDAS {
            let c = m.codigo_padrao();
            assert!(formatar(&Value::Number(-1.0), &c).is_some(), "{}", m.nome);
            assert_eq!(do_codigo(&c).map(|x| x.etiqueta()), Some(m.etiqueta()), "{}", m.nome);
        }
    }
}
