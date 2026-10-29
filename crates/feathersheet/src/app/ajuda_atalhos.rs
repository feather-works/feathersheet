//! UI-013: atalhos originais do FeatherSheet TS: fixos e configuráveis.
//! Nota: a GUI Iced não implementa ainda a edição de atalhos do localStorage TS.
use feather_ui::ajuda::{Linha,Topico};
use serde_json::Value;
const FIXOS: &[(&str,&str)] = &[
    ("ajuda","F1"),
    ("faixa","Ctrl+F1"),
    ("editar","F2"),
    ("confirmar","Enter / Tab"),
    ("fimDosDados","Ctrl + ←↑→↓"),
    ("selecionarAteOFim","Ctrl+Shift + ←↑→↓"),
    ("desfazer","Ctrl+Z / Ctrl+Y"),
    ("copiar","Ctrl+C / Ctrl+X / Ctrl+V"),
    ("negrito","Ctrl+B / Ctrl+I / Ctrl+U"),
    ("formatarCelulas","Ctrl+1"),
    ("imprimir","Ctrl+P"),
    ("monitor","F12"),
    ("executarMacro","Ctrl+Enter"),
];
const CONFIGURAVEIS: &[(&str,&str)] = &[
    ("createTable","Ctrl+T"),
    ("toggleFilter","Ctrl+Shift+L"),
    ("findReplace","Ctrl+H"),
    ("memoryProfiler","Ctrl+Shift+M"),
    ("save","Ctrl+S"),
    ("saveAs","Ctrl+Shift+S"),
    ("toggleFormulas","Ctrl+`"),
    ("selecionarColuna","Ctrl+Space"),
    ("selecionarLinha","Shift+Space"),
    ("preencherBaixo","Ctrl+D"),
    ("preencherDireita","Ctrl+R"),
    ("dataDeHoje","Ctrl+;"),
    ("horaAgora","Ctrl+Shift+:"),
    ("autoSoma","Alt+="),
    ("ocultarLinha","Ctrl+9"),
    ("ocultarColuna","Ctrl+0"),
    ("reexibirLinhas","Ctrl+Shift+("),
    ("reexibirColunas","Ctrl+Shift+)"),
    ("formatoMoeda","Ctrl+Shift+$"),
    ("formatoPorcentagem","Ctrl+Shift+%"),
    ("proximaAba","Ctrl+PageDown"),
    ("abaAnterior","Ctrl+PageUp"),
    ("novaAba","Shift+F11"),
    ("colarValores","Ctrl+Shift+V"),
];
pub fn montar(json:&Value)->Vec<Topico>{
    vec![
        Topico {
            id:"fixos".into(),
            titulo:json["ajuda"]["rotulos"]["fixos"].as_str().unwrap_or(tr!("Fixos")).into(),
            linhas:FIXOS.iter().map(|(id,tecla)|Linha::Atalho {
                descricao:json["ajuda"]["atalhosFixos"][*id].as_str().unwrap_or(id).into(),
                tecla:(*tecla).into(),
            }).collect()
        },
        Topico {
            id:"configuraveis".into(),
            titulo:json["ajuda"]["rotulos"]["configuraveis"].as_str()
                .unwrap_or(tr!("Configuráveis (padrões originais)")).into(),
            linhas:CONFIGURAVEIS.iter().map(|(id,tecla)|Linha::Atalho {
                descricao:json["atalhos"][*id].as_str().unwrap_or(id).into(),
                tecla:(*tecla).into(),
            }).collect()
        }
    ]
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test] fn replica_atalhos_typescript() {
        assert_eq!(FIXOS.len(),13);
        assert_eq!(CONFIGURAVEIS.len(),24);
        let j:Value=serde_json::from_str(include_str!("../../../../src/idiomas/pt-BR.json")).unwrap();
        for (id,_) in FIXOS {assert!(j["ajuda"]["atalhosFixos"][*id].is_string());}
        for (id,_) in CONFIGURAVEIS {assert!(j["atalhos"][*id].is_string());}
        assert_eq!(montar(&j).len(),2);
    }
}
