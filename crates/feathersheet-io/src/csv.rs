//! Adaptador CSV/TSV do FeatherSheet para o parser compartilhado FeatherWorks.
//!
//! O parser permissivo e a divisão em lotes foram movidos para
//! `feather_io::csv_lotes`. Esta reexportação mantém a assinatura
//! usada por importação de arquivos, área de transferência e conversores
//! do Sheet sem cópia ou materialização adicional.
//!
//! Delimitadores, aspas, multilinhas e campos em locale PT-BR preservam
//! exatamente os valores textuais para o motor decidir tipos e formatos.

pub use feather_io::csv_lotes::{
    detectar_separador, dividir_csv, escolher_separador, dividir_csv_em_lotes, dividir_csv_linha,
    escrever_campo, para_delimitado, LINHAS_POR_LOTE, SEM_SEPARADOR,
};

#[cfg(test)]
mod testes {
    use super::*;

    #[test]
    fn o_separador_e_o_mais_frequente_da_primeira_linha() {
        assert_eq!(detectar_separador("a;b;c\n1,5;2;3"), ';');
        assert_eq!(detectar_separador("a\tb\n"), '\t');
        assert_eq!(detectar_separador("só um campo"), ',');
    }

    #[test]
    fn aspas_protegem_separador_quebra_e_aspas() {
        let l = dividir_csv("a;\"b;c\";\"d\"\"e\"\r\n\"x\ny\";2\n", ';');
        assert_eq!(l, vec![vec!["a", "b;c", "d\"e"], vec!["x\ny", "2"]]);
    }

    #[test]
    fn em_lotes_da_o_mesmo_que_inteiro() {
        let texto = "a;\"b;c\";\"d\"\"e\"\r\n\"x\ny\";2\n1;2;3\n4;5";
        let inteiro = dividir_csv(texto, ';');
        let mut juntos = Vec::new();
        let mut tamanhos = Vec::new();
        dividir_csv_em_lotes(texto, ';', 2, |l| {
            tamanhos.push(l.len());
            juntos.append(l);
        });
        assert_eq!(juntos, inteiro);
        assert_eq!(tamanhos, [2, 2], "nenhum lote passa do tamanho pedido");
    }

    #[test]
    fn empate_fica_com_a_virgula_como_no_typescript() {
        assert_eq!(detectar_separador("x;y,z\n1;2,3"), ',');
        assert_eq!(detectar_separador("a;b;c,d"), ';');
        assert_eq!(detectar_separador("a\tb\r\n1,2,3,4"), '\t');
        assert_eq!(detectar_separador("sem separador"), ',');
    }
}
