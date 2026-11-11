//! Cálculo manual (Fórmulas → Opções de cálculo), F9 e Shift+F9.
//!
//! No modo manual a escrita continua mudando a célula escrita (a fórmula nova
//! calcula ao ser lida, como no Excel), mas a cascata de "sujo" para quem
//! depende dela fica guardada: na aba, em `Sheet::adiadas`; entre abas, em
//! `Workbook::externas_adiadas`. O F9 refaz essas cascatas de uma vez — só o
//! que mudou, não a pasta inteira.
use crate::planilha::sheet::Sheet;
use crate::planilha::workbook::Workbook;

impl Sheet {
    /// Refaz as cascatas guardadas no modo manual. Devolve se havia alguma.
    fn soltar_adiadas(&self) -> bool {
        let chaves = std::mem::take(&mut *self.adiadas.borrow_mut());
        if chaves.is_empty() {
            return false;
        }
        let manual = self.manual.replace(false);
        self.sujar_celulas(&chaves);
        self.manual.set(manual);
        true
    }

    /// Há mudança esperando o F9 nesta aba.
    pub fn calculo_adiado(&self) -> bool {
        !self.adiadas.borrow().is_empty()
    }
}

impl Workbook {
    pub fn calculo_manual(&self) -> bool {
        self.manual
    }

    /// Liga ou desliga o cálculo manual. Voltar ao automático calcula o que
    /// ficou esperando, como o Excel.
    pub fn definir_calculo_manual(&mut self, manual: bool) {
        self.manual = manual;
        for i in 0..self.len() {
            if let Some(aba) = self.aba_mut(i) {
                aba.sheet.manual.set(manual);
            }
        }
        if !manual {
            self.calcular_agora();
        }
    }

    /// Há mudança esperando o F9 em alguma aba (o rodapé mostra "Calcular").
    pub fn calculo_pendente_manual(&self) -> bool {
        !self.externas_adiadas.is_empty()
            || (0..self.len()).any(|i| self.aba(i).is_some_and(|a| a.sheet.calculo_adiado()))
    }

    /// F9: calcula tudo o que mudou desde o último cálculo, em todas as abas.
    pub fn calcular_agora(&mut self) {
        for i in 0..self.len() {
            if let Some(aba) = self.aba_mut(i) {
                aba.sheet.sujar_volateis();
            }
            self.calcular_aba(i);
        }
        let manual = std::mem::replace(&mut self.manual, false);
        for (mexida, alcance) in std::mem::take(&mut self.externas_adiadas) {
            self.depois_de_mudar_em(mexida, alcance.as_deref());
        }
        self.manual = manual;
    }

    /// Shift+F9: só a aba `indice`.
    pub fn calcular_aba(&mut self, indice: usize) {
        if let Some(aba) = self.aba_mut(indice) {
            aba.sheet.sujar_volateis();
            aba.sheet.sujar_ciclos();
            if aba.sheet.soltar_adiadas() {
                aba.versao = crate::planilha::workbook::proxima_versao(aba.versao);
            }
        }
    }
}

#[cfg(test)]
mod testes {
    use crate::celulas::values::Value;
    use crate::planilha::workbook::Workbook;

    fn n(v: f64) -> Value {
        Value::Number(v)
    }

    #[test]
    fn manual_espera_o_f9_e_formula_nova_calcula_na_hora() {
        let mut w = Workbook::new();
        w.escrever_celula(0, 0, 0, "2".into());
        w.escrever_celula(0, 0, 1, "=A1*10".into());
        assert_eq!(w.valor(0, 0, 1), n(20.0));
        w.definir_calculo_manual(true);
        w.escrever_celula(0, 0, 0, "3".into());
        assert_eq!(w.valor(0, 0, 0), n(3.0), "a célula escrita muda");
        assert_eq!(w.valor(0, 0, 1), n(20.0), "quem depende espera o F9");
        assert!(w.calculo_pendente_manual());
        // Fórmula nova calcula na hora, com os valores de agora.
        w.escrever_celula(0, 1, 1, "=A1+1".into());
        assert_eq!(w.valor(0, 1, 1), n(4.0));
        w.calcular_agora();
        assert_eq!(w.valor(0, 0, 1), n(30.0));
        assert!(!w.calculo_pendente_manual());
    }

    #[test]
    fn volateis_so_mudam_em_recalculo_da_pasta() {
        let mut w = Workbook::new();
        w.escrever_celula(0, 0, 0, "=ALEATÓRIO()".into());
        w.escrever_celula(0, 0, 2, "=A1*2".into());
        let v0 = w.valor(0, 0, 0);
        let d0 = w.valor(0, 0, 2);
        for _ in 0..50 {
            assert_eq!(w.valor(0, 0, 0), v0, "leitura repetida não sorteia");
            assert_eq!(w.valor(0, 0, 2), d0);
            let _ = w.aba(0).unwrap().sheet.calculo_pendente(0, 0);
        }
        w.escrever_celula(0, 5, 5, "7".into());
        let v1 = w.valor(0, 0, 0);
        assert_ne!(v1, v0, "editar outra célula recalcula a pasta");
        assert_eq!(w.valor(0, 0, 0), v1);
        w.calcular_agora();
        let v2 = w.valor(0, 0, 0);
        assert_ne!(v2, v1, "F9 sorteia de novo");
        assert_eq!(w.valor(0, 0, 0), v2);
        assert_eq!(w.valor(0, 0, 2), Value::Number(match v2 { Value::Number(x) => x * 2.0, _ => f64::NAN }));
    }

    #[test]
    fn manual_entre_abas_e_volta_ao_automatico() {
        let mut w = Workbook::new();
        let dados = w.criar_aba("Dados");
        w.escrever_celula(dados, 0, 0, "5".into());
        w.escrever_celula(0, 0, 0, "=Dados!A1*2".into());
        assert_eq!(w.valor(0, 0, 0), n(10.0));
        w.definir_calculo_manual(true);
        w.escrever_celula(dados, 0, 0, "6".into());
        assert_eq!(w.valor(0, 0, 0), n(10.0));
        // Shift+F9 na aba Dados não mexe na outra.
        w.calcular_aba(dados);
        assert_eq!(w.valor(0, 0, 0), n(10.0));
        w.definir_calculo_manual(false);
        assert_eq!(w.valor(0, 0, 0), n(12.0));
    }
}
