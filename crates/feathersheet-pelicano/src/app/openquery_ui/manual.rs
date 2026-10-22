//! Entrada manual de dados. Reutiliza parser CSV/TSV compartilhado.
//! Dados iniciados por '=' permanecem texto, nunca fórmula.
use super::*;
const MAX_TEXTO_MANUAL: usize = 2 * 1024 * 1024;

pub(super) fn ler(texto: &str) -> Result<Tabela, String> {
    if texto.trim().is_empty() {
        return Err(tr!("Cole uma tabela com cabeçalhos na primeira linha").into());
    }
    if texto.len() > MAX_TEXTO_MANUAL {
        return Err(tr!("Entrada manual acima de 2 MiB; importe um arquivo").into());
    }
    let separador = if texto.lines().next().unwrap_or("").contains('\t') {
        '\t'
    } else {
        feathersheet_io::csv::detectar_separador(texto)
    };
    let mut linhas = feathersheet_io::csv::dividir_csv(texto, separador);
    if linhas.is_empty() { return Err(tr!("Tabela manual vazia").into()); }
    let cabecalho = linhas.remove(0).into_iter().enumerate()
        .map(|(i,nome)| if nome.trim().is_empty() {
            trf!("Coluna {}",i+1)
        } else { nome }).collect::<Vec<_>>();
    let largura = cabecalho.len();
    if largura == 0 { return Err(tr!("Cabeçalho vazio").into()); }
    let celulas = (linhas.len() as u64+1).saturating_mul(largura as u64);
    if celulas > MAX_CELULAS_FONTE {
        return Err(trf!("Entrada manual de {celulas} células excede o limite de {MAX_CELULAS_FONTE}"));
    }
    let mut out = Vec::with_capacity(linhas.len());
    for (i,linha) in linhas.into_iter().enumerate() {
        if linha.len() > largura {
            return Err(trf!("Linha {} possui {} colunas, mas o cabeçalho possui {}",
                i+2, linha.len(), largura));
        }
        let mut valores = linha.into_iter().map(|v| if v.is_empty() {
            Value::Empty
        } else { Value::Text(v) }).collect::<Vec<_>>();
        valores.resize(largura,Value::Empty);
        out.push(valores);
    }
    Ok(Tabela { cabecalho, linhas: out })
}
#[cfg(test)]
mod testes {
    use super::*;
    #[test]
    fn interpreta_aspas_e_preserva_texto_de_formula() {
        let t=ler("Nome;Código\n\"Vitória;ES\";=A1\n").unwrap();
        assert_eq!(t.cabecalho,vec!["Nome","Código"]);
        assert_eq!(t.linhas[0][0],Value::Text("Vitória;ES".into()));
        assert_eq!(t.linhas[0][1],Value::Text("=A1".into()));
    }
    #[test]
    fn tabulacao_e_linha_curta_preservam_vazio() {
        let t=ler("A\tB\tC\n1\t2\n").unwrap();
        assert_eq!(t.linhas[0][2],Value::Empty);
    }
    #[test]
    fn largura_irregular_e_rejeitada() {
        assert!(ler("A;B\n1;2;3\n").unwrap_err().contains("Linha 2"));
    }
}
