//! O que uma macro faz fora da planilha, para o aviso "Ativar macros?". Só lê
//! o texto, não roda nada. A macro não acessa rede nem apaga arquivo (a gaiola
//! não deixa); o que sai dela é gravar PDF/.xlsx/.fshz na pasta da planilha e
//! pedir programas (`app.shell`), que o app só roda depois de perguntar. Os comandos perigosos do VBA original ficam nos comentários
//! `// VBA não convertido`, e avisar deles mostra de onde o arquivo veio.

/// Palavras achadas no código, sem repetir.
#[derive(Debug, Default, PartialEq)]
pub struct Riscos {
    /// Chamadas que gravam arquivo (`exportPdf`, `saveSheetsAs`, `saveAs`).
    pub grava: Vec<&'static str>,
    /// Chamadas que pedem para rodar programa (`app.shell`).
    pub executa: Vec<&'static str>,
    /// VBA não convertido que chamava programa ou internet (`Shell`, `CreateObject`…).
    pub vba_fora: Vec<&'static str>,
    /// VBA não convertido que apagava arquivo (`Kill`, `DeleteFile`…).
    pub vba_apaga: Vec<&'static str>,
    /// Sinais de binário guardado como texto no código (executável em Base64/hex, lista
    /// de bytes, `certutil -decode`…). Só avisa: não é antivírus nem diz se é vírus.
    pub binario: Vec<String>,
}

const GRAVA: &[&str] = &["exportPdf", "saveSheetsAs", "saveAs"];
const EXECUTA: &[&str] = &["shell"];
const VBA_FORA: &[&str] = &["Shell", "CreateObject", "GetObject", "Declare", "URLDownloadToFile",
    "WScript", "PowerShell", "SendKeys"];
const VBA_APAGA: &[&str] = &["Kill", "RmDir", "DeleteFile", "DeleteFolder"];

impl Riscos {
    pub fn vazio(&self) -> bool {
        self.grava.is_empty() && self.executa.is_empty() && self.vba_fora.is_empty() && self.vba_apaga.is_empty()
            && self.binario.is_empty()
    }

    /// Soma o que achou em `codigo` (chamar uma vez por módulo).
    pub fn ler(&mut self, codigo: &str) {
        for motivo in binario_escondido(codigo) {
            if !self.binario.contains(&motivo) { self.binario.push(motivo); }
        }
        for linha in codigo.lines() {
            let (codigo, comentario) = match linha.find("//") {
                Some(i) => (&linha[..i], &linha[i..]),
                None => (linha, ""),
            };
            for palavra in palavras(codigo) {
                anotar(&mut self.grava, GRAVA, palavra, false);
                anotar(&mut self.executa, EXECUTA, palavra, false);
            }
            for palavra in palavras(comentario) {
                anotar(&mut self.vba_fora, VBA_FORA, palavra, true);
                anotar(&mut self.vba_apaga, VBA_APAGA, palavra, true);
            }
        }
    }
}

/// Texto codificado somado acima disto (~300 bytes) já é arquivo, não dado de planilha.
const TETO_CODIFICADO: usize = 400;
/// Um trecho só conta a partir disto (nome de função comprido não chega lá).
const TRECHO_MINIMO: usize = 40;
/// Lista de números 0–255 seguidos, ou `Chr(…)`, a partir disto.
const TETO_BYTES: usize = 64;

/// Comandos que decodificam, baixam ou rodam o que veio escondido.
const DECODIFICA: &[&str] = &["certutil", "-encodedcommand", "frombase64string", "adodb.stream",
    "savetofile", "for binary", "mshta", "regsvr32", "rundll32", "bitsadmin", "invoke-expression",
    "downloadfile", "base64 -d", "base64 --decode", "xxd -r", "chmod +x"];

/// Os sinais de programa escondido em `texto` (código da macro, comentário do VBA ou o
/// comando do `Shell`), em frases para a pessoa ler. Vazio se não achou nada.
/// shortcut: heurística (não é antivírus), pega o dropper comum; ofuscação de verdade passa.
pub fn binario_escondido(texto: &str) -> Vec<String> {
    let mut motivos = Vec::new();
    let (mut b64, mut hex) = (0usize, 0usize);
    let (mut exe_b64, mut exe_hex) = (false, false);
    for trecho in texto.split(|c: char| !(c.is_ascii_alphanumeric() || matches!(c, '+' | '/' | '='))) {
        if trecho.len() < TRECHO_MINIMO || !trecho.bytes().any(|b| b.is_ascii_digit()) { continue; }
        if trecho.bytes().all(|b| b.is_ascii_hexdigit()) {
            hex += trecho.len();
            let t = trecho.to_ascii_uppercase();
            exe_hex |= t.starts_with("4D5A") || t.starts_with("7F454C46");
        } else {
            b64 += trecho.len();
            exe_b64 |= ["TVqQ", "TVpQ", "TVoA", "f0VMR"].iter().any(|m| trecho.starts_with(m));
        }
    }
    if exe_b64 || exe_hex {
        motivos.push(format!("um programa executável escondido como texto ({})", if exe_b64 { "Base64" } else { "hexadecimal" }));
    } else if b64 + hex >= TETO_CODIFICADO {
        motivos.push(format!("{} KB de texto codificado (Base64 ou hexadecimal), o formato de um arquivo escondido", (b64 + hex).div_ceil(1024)));
    }
    let (bytes, exe) = maior_lista_de_bytes(texto);
    if exe {
        motivos.push("um programa executável escondido como lista de números (bytes)".into());
    } else if bytes >= TETO_BYTES {
        motivos.push(format!("uma lista de {bytes} números de 0 a 255 seguidos, o formato de um arquivo escondido"));
    }
    let minusculo = texto.to_lowercase();
    let chr = minusculo.matches("chr(").count() + minusculo.matches("chrw(").count() + minusculo.matches("fromcharcode(").count();
    if chr >= TETO_BYTES {
        motivos.push(format!("{chr} caracteres montados um a um (Chr), jeito comum de esconder comando"));
    }
    let achados: Vec<&str> = DECODIFICA.iter().copied().filter(|k| minusculo.contains(k)).collect();
    // "-enc" sozinho também é o atalho do PowerShell para -EncodedCommand.
    let enc = minusculo.contains("powershell") && (minusculo.contains(" -enc ") || minusculo.contains(" -e "));
    if !achados.is_empty() || enc {
        let mut lista = achados.join(", ");
        if enc { if !lista.is_empty() { lista.push_str(", "); } lista.push_str("powershell -enc"); }
        motivos.push(format!("comandos que decodificam ou rodam arquivo escondido ({lista})"));
    }
    motivos
}

/// A maior sequência de números 0–255 separados por vírgula, e se começa com a
/// assinatura de executável (77, 90 = "MZ" do Windows; 127, 69, 76, 70 = ELF).
fn maior_lista_de_bytes(texto: &str) -> (usize, bool) {
    let (mut maior, mut exe) = (0, false);
    let mut atual: Vec<u32> = Vec::new();
    let mut fechar = |atual: &mut Vec<u32>| {
        if atual.len() > maior { maior = atual.len(); }
        if atual.len() >= 16 && (atual.starts_with(&[77, 90]) || atual.starts_with(&[127, 69, 76, 70])) { exe = true; }
        atual.clear();
    };
    for parte in texto.split(',') {
        // "_" e quebra de linha: a continuação de linha do VBA no meio da lista.
        let p = parte.trim_matches(|c: char| c.is_whitespace() || c == '_');
        let numero = p.trim_start_matches(|c: char| !c.is_ascii_digit());
        let comeco_ok = atual.is_empty() || numero.len() == p.len();
        match numero.parse::<u32>() {
            Ok(n) if n <= 255 && comeco_ok => {
                // O primeiro número pode vir colado em "Array(" ou "[".
                atual.push(n);
            }
            _ => {
                // O último número pode vir colado em ")" ou "]".
                let fim: String = p.chars().take_while(char::is_ascii_digit).collect();
                if let Ok(n) = fim.parse::<u32>() { if n <= 255 && !atual.is_empty() { atual.push(n); } }
                fechar(&mut atual);
                let ini = p.rsplit(|c: char| !c.is_ascii_digit()).next().unwrap_or("");
                if let Ok(n) = ini.parse::<u32>() { if n <= 255 && !ini.is_empty() && p.len() > ini.len() { atual.push(n); } }
            }
        }
    }
    fechar(&mut atual);
    (maior, exe)
}

fn palavras(texto: &str) -> impl Iterator<Item = &str> {
    texto.split(|c: char| !(c.is_alphanumeric() || c == '_')).filter(|p| !p.is_empty())
}

/// O VBA não diferencia maiúsculas; o JavaScript sim.
fn anotar(lista: &mut Vec<&'static str>, conhecidas: &[&'static str], palavra: &str, vba: bool) {
    let achada = conhecidas.iter().find(|k| if vba { k.eq_ignore_ascii_case(palavra) } else { **k == palavra });
    if let Some(k) = achada.filter(|k| !lista.contains(k)) { lista.push(k); }
}

#[cfg(test)]
mod testes {
    use super::*;

    #[test]
    fn acha_gravacao_e_vba_perigoso_so_onde_conta() {
        let mut r = Riscos::default();
        r.ler(r#"sheet.exportPdf("a.pdf"); // shell aqui é só comentário de quem escreveu
// VBA não convertido (API do Windows):
//   Kill "C:\x.txt"
//   Call Shell("cmd /c del *.*")
const Killer = 1; wb.sheet("A").saveAs("b.fshz");"#);
        assert_eq!(r.grava, ["exportPdf", "saveAs"]);
        assert_eq!(r.vba_fora, ["Shell"]);
        assert_eq!(r.vba_apaga, ["Kill"]);
        assert!(r.executa.is_empty(), "o Shell só no comentário não roda");
        let mut roda = Riscos::default();
        roda.ler(r#"app.shell("notepad.exe");"#);
        assert_eq!(roda.executa, ["shell"]);
        assert!(r.binario.is_empty(), "{:?}", r.binario);
        let mut limpo = Riscos::default();
        limpo.ler("sheet.set('A1', 'Shell'); for (;;) {}");
        assert!(limpo.vazio());
    }

    #[test]
    fn acha_programa_escondido_em_base64_hex_bytes_e_comandos() {
        let b64 = "const s = \"TVqQAAMAAAAEAAAA//8AALgAAAAAAAAAQAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA\";";
        assert!(binario_escondido(b64)[0].contains("executável"), "{:?}", binario_escondido(b64));
        assert!(binario_escondido("x = \"4D5A90000300000004000000FFFF0000B8000000000000004000\"")[0].contains("hexadecimal"));
        let longo: String = (0..12).map(|i| format!("s = s & \"QUJDREVGR0hJSktMTU5PUFFSU1RVVldYWVo{i}xMjM0NTY3ODk\"\n")).collect();
        assert!(binario_escondido(&longo)[0].contains("KB de texto codificado"), "{:?}", binario_escondido(&longo));
        let bytes: Vec<String> = [77, 90, 144, 0].iter().cycle().take(80).map(u32::to_string).collect();
        let lista = format!("const b = [{}];", bytes.join(", "));
        assert!(binario_escondido(&lista).iter().any(|m| m.contains("lista de números")), "{:?}", binario_escondido(&lista));
        let vba = format!("// VBA não convertido:\n//   b = Array({}, _\n//   1, 2)", bytes.join(","));
        assert!(binario_escondido(&vba).iter().any(|m| m.contains("lista de números")));
        let chr: String = (0..70).map(|_| "Chr(65) & ").collect();
        assert!(binario_escondido(&chr).iter().any(|m| m.contains("Chr")));
        let ps = binario_escondido("app.shell(\"powershell -w hidden -enc SQBFAFgA\")");
        assert!(ps.iter().any(|m| m.contains("powershell -enc")), "{ps:?}");
        assert!(binario_escondido("app.shell(\"certutil -decode a.txt a.exe\")")[0].contains("certutil"));
    }

    #[test]
    fn planilha_comum_nao_acusa() {
        let comum = r##"
            const meses = [31, 28, 31, 30, 31, 30, 31, 31, 30, 31, 30, 31];
            sheet.set("A1", "=SOMA(B2:B200)"); sheet.style("A1:C3", { bg: "#ffeecc" });
            const id = "550e8400-e29b-41d4-a716-446655440000";
            for (let i = 1; i <= 100; i++) sheet.set(`A${i}`, i * 2);
            app.shell("notepad.exe relatorio.txt");
            // VBA não convertido: Range("A1:Z1000").Value = Application.Transpose(dados)
        "##;
        assert!(binario_escondido(comum).is_empty(), "{:?}", binario_escondido(comum));
    }
}
