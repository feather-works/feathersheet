//! O que uma macro faz fora da planilha, para o aviso "Ativar macros?". Só lê
//! o texto, não roda nada. A macro não chama programa, rede nem apaga arquivo
//! (a gaiola não deixa); o que sai dela é gravar PDF/.xlsx/.fshz na pasta da
//! planilha. Os comandos perigosos do VBA original ficam nos comentários
//! `// VBA não convertido`, e avisar deles mostra de onde o arquivo veio.

/// Palavras achadas no código, sem repetir.
#[derive(Debug, Default, PartialEq)]
pub struct Riscos {
    /// Chamadas que gravam arquivo (`exportPdf`, `saveSheetsAs`, `saveAs`).
    pub grava: Vec<&'static str>,
    /// VBA não convertido que chamava programa ou internet (`Shell`, `CreateObject`…).
    pub vba_fora: Vec<&'static str>,
    /// VBA não convertido que apagava arquivo (`Kill`, `DeleteFile`…).
    pub vba_apaga: Vec<&'static str>,
}

const GRAVA: &[&str] = &["exportPdf", "saveSheetsAs", "saveAs"];
const VBA_FORA: &[&str] = &["Shell", "CreateObject", "GetObject", "Declare", "URLDownloadToFile",
    "WScript", "PowerShell", "SendKeys"];
const VBA_APAGA: &[&str] = &["Kill", "RmDir", "DeleteFile", "DeleteFolder"];

impl Riscos {
    pub fn vazio(&self) -> bool {
        self.grava.is_empty() && self.vba_fora.is_empty() && self.vba_apaga.is_empty()
    }

    /// Soma o que achou em `codigo` (chamar uma vez por módulo).
    pub fn ler(&mut self, codigo: &str) {
        for linha in codigo.lines() {
            let (codigo, comentario) = match linha.find("//") {
                Some(i) => (&linha[..i], &linha[i..]),
                None => (linha, ""),
            };
            for palavra in palavras(codigo) {
                anotar(&mut self.grava, GRAVA, palavra, false);
            }
            for palavra in palavras(comentario) {
                anotar(&mut self.vba_fora, VBA_FORA, palavra, true);
                anotar(&mut self.vba_apaga, VBA_APAGA, palavra, true);
            }
        }
    }
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
        let mut limpo = Riscos::default();
        limpo.ler("sheet.set('A1', 'Shell'); for (;;) {}");
        assert!(limpo.vazio());
    }
}
