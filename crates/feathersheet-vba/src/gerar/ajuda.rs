//! As funções `vba.*` que o código gerado usa: o que o VBA faz de um jeito que o
//! JavaScript não faz igual (comparar e somar Variant, `Mid` contando do 1, arredondar
//! para o par, `Format`, `MsgBox`). Cada macro leva só as que usa — e as que essas usam.

use std::collections::BTreeSet;

/// (nome, dependências, código do método no objeto `vba`).
const AJUDAS: &[(&str, &[&str], &str)] = &[
    ("vazio", &[], "  /** Empty do VBA: célula vazia, variável sem valor. */\n  vazio(v) { return v === null || v === undefined || v === ''; },"),
    ("str", &[], "  /** O valor como texto, como o CStr do VBA (Empty vira \"\", True vira \"True\"): número com até 15 algarismos, \"1E+15\" e o separador decimal da língua do app (0,1 + 0,2 é \"0,3\"). */\n  str(v) { if (v === null || v === undefined) return ''; if (typeof v === 'boolean') return v ? 'True' : 'False'; if (typeof v !== 'number') return String(v); if (!Number.isFinite(v) || v === 0) return String(v); vba._sep ??= app.decimalSeparator || ','; const a = Math.abs(v); let s; if (a >= 1e15 || a < 1e-6) { const [m, e] = v.toExponential(14).split('e'); const n = Number(e); s = `${m.replace(/\\.?0+$/, '')}E${n < 0 ? '-' : '+'}${String(Math.abs(n)).padStart(2, '0')}`; } else s = String(Number(v.toPrecision(15))); return vba._sep === '.' ? s : s.replace('.', vba._sep); },"),
    ("numero", &[], r#"  /** Texto que é número (aceita vírgula ou ponto decimal); NaN se não for. Sem regex: no motor das macros ela é recompilada a cada chamada. */
  numero(v) { if (typeof v === 'number') return v; const t = String(v).trim(); const n = t.length; let i = 0, dig = 0, sep = -1; if (t[0] === '+' || t[0] === '-') i++; for (; i < n; i++) { const c = t.charCodeAt(i); if (c >= 48 && c <= 57) dig++; else if ((c === 44 || c === 46) && sep < 0) sep = i; else break; } if (!dig) return NaN; if (i < n) { if (t[i] !== 'e' && t[i] !== 'E') return NaN; let j = i + 1; if (t[j] === '+' || t[j] === '-') j++; const k = j; while (j < n && t.charCodeAt(j) >= 48 && t.charCodeAt(j) <= 57) j++; if (j === k || j < n) return NaN; } return Number(sep < 0 ? t : t.slice(0, sep) + '.' + t.slice(sep + 1)); },"#),
    ("n", &["vazio", "numero"], "  /** O valor como número, como o VBA: Empty é 0, True é -1; texto que não é número dá o erro 13. */\n  n(v) { if (typeof v === 'number') return v; if (vba.vazio(v)) return 0; if (typeof v === 'boolean') return v ? -1 : 0; const x = vba.numero(v); if (Number.isNaN(x)) throw new TypeError(`Tipo incompatível: \"${v}\" não é número`); return x; },"),
    ("cat", &["str"], "  /** O `&` do VBA: junta como texto. */\n  cat(...p) { return p.map(vba.str).join(''); },"),
    ("soma", &["n"], "  /** O `+` do VBA com Variant: dois textos se juntam, o resto soma. */\n  soma(a, b) { return typeof a === 'string' && typeof b === 'string' ? a + b : vba.n(a) + vba.n(b); },"),
    ("cmp", &["vazio", "numero", "str"], "  /** Compara dois Variant como o VBA: -1, 0 ou 1. Número contra texto numérico compara como número. */\n  cmp(a, b) { if (typeof a === typeof b && (typeof a === 'number' || typeof a === 'string')) return a < b ? -1 : a > b ? 1 : 0; const lg = (x) => (typeof x === 'string' && /^(true|verdadeiro)$/i.test(x.trim()) ? true : typeof x === 'string' && /^(false|falso)$/i.test(x.trim()) ? false : x); if (typeof a === 'boolean' || typeof b === 'boolean') { a = lg(a); b = lg(b); } if (typeof a === 'boolean') a = a ? -1 : 0; if (typeof b === 'boolean') b = b ? -1 : 0; if (vba.vazio(a) && vba.vazio(b)) return 0; if (typeof a === 'number' || typeof b === 'number') { const x = vba.vazio(a) ? 0 : vba.numero(a); const y = vba.vazio(b) ? 0 : vba.numero(b); if (!Number.isNaN(x) && !Number.isNaN(y)) return x < y ? -1 : x > y ? 1 : 0; return typeof a === 'number' ? -1 : 1; } const s = vba.str(a); const t = vba.str(b); return s < t ? -1 : s > t ? 1 : 0; },"),
    ("eq", &["cmp"], "  /** O `=` do VBA entre dois Variant. */\n  eq(a, b) { if (typeof a === typeof b && (typeof a === 'string' || typeof a === 'number')) return a === b; return vba.cmp(a, b) === 0; },"),
    ("bool", &["vazio", "numero"], "  /** O valor como verdadeiro/falso, como o `If` do VBA. */\n  bool(v) { if (typeof v === 'boolean') return v; if (vba.vazio(v)) return false; if (typeof v === 'number') return v !== 0; const t = String(v).trim().toLowerCase(); if (t === 'true' || t === 'verdadeiro') return true; if (t === 'false' || t === 'falso') return false; const x = vba.numero(t); if (Number.isNaN(x)) throw new TypeError(`Tipo incompatível: \"${v}\" não é verdadeiro/falso`); return x !== 0; },"),
    ("len", &["str"], "  len(v) { return vba.str(v).length; },"),
    ("mid", &["str", "n"], "  /** Mid(texto, início, [quantos]) — começa do 1. */\n  mid(s, i, q) { const t = vba.str(s); const a = Math.max(0, vba.n(i) - 1); return q === undefined ? t.slice(a) : t.slice(a, a + Math.max(0, vba.n(q))); },"),
    ("left", &["str", "n"], "  left(s, q) { return vba.str(s).substring(0, Math.max(0, vba.n(q))); },"),
    ("right", &["str", "n"], "  right(s, q) { const t = vba.str(s); return t.substring(Math.max(0, t.length - vba.n(q))); },"),
    ("instr", &["str", "n"], "  /** InStr([início,] texto, procurado): a posição (do 1) ou 0. */\n  instr(a, b, c) { let ini = 1; if (c !== undefined) { ini = vba.n(a); a = b; b = c; } return vba.str(a).indexOf(vba.str(b), ini - 1) + 1; },"),
    ("instrRev", &["str", "n"], "  /** InStrRev(texto, procurado, [início], [modo]): a posição (do 1) procurando do fim; vbTextCompare (1) ignora maiúsculas. */\n  instrRev(s, b, ini = -1, modo = 0) { let t = vba.str(s), p = vba.str(b); if (vba.n(modo) === 1) { t = t.toLowerCase(); p = p.toLowerCase(); } const i = vba.n(ini); return t.lastIndexOf(p, i < 0 ? Infinity : i - p.length) + 1; },"),
    ("replace", &["str", "n"], "  /** Replace(texto, de, para, [início], [quantas], [modo]): como no VBA, o resultado começa em `início`; quantas -1 = todas. */\n  replace(s, de, para, ini = 1, qtd = -1, modo = 0) { let t = vba.str(s).substring(vba.n(ini) - 1); const d = vba.str(de), p = vba.str(para); let q = vba.n(qtd); if (!d) return t; const alvo = vba.n(modo) === 1 ? t.toLowerCase() : t, dd = vba.n(modo) === 1 ? d.toLowerCase() : d; let r = '', i = 0, j; while (q !== 0 && (j = alvo.indexOf(dd, i)) >= 0) { r += t.slice(i, j) + p; i = j + d.length; q--; } return r + t.slice(i); },"),
    ("trim", &["str"], r#"  /** Trim do VBA: só espaço, não tabulação. */
  trim(s) { s = vba.str(s); let a = 0, b = s.length; while (a < b && s.charCodeAt(a) === 32) a++; while (b > a && s.charCodeAt(b - 1) === 32) b--; return s.slice(a, b); },"#),
    ("ltrim", &["str"], "  ltrim(s) { return vba.str(s).replace(/^ +/, ''); },"),
    ("rtrim", &["str"], "  rtrim(s) { return vba.str(s).replace(/ +$/, ''); },"),
    ("isNumeric", &["numero"], "  isNumeric(v) { if (v === null || v === undefined || typeof v === 'number' || typeof v === 'boolean') return true; return !Number.isNaN(vba.numero(v)); },"),
    ("val", &["str"], "  /** Val: o número do começo do texto (ponto decimal, ignora espaços). */\n  val(s) { const m = vba.str(s).replace(/\\s+/g, '').match(/^[+-]?(\\d+\\.?\\d*|\\.\\d+)(e[+-]?\\d+)?/i); return m ? Number(m[0]) : 0; },"),
    ("round", &["n", "potencias"], "  /** Arredonda para o par no empate (o Round/CInt do VBA): 2,5 → 2; 3,5 → 4. */\n  round(x, casas = 0) { if (typeof x === 'number' && typeof casas === 'number') { const f = vba._pot[casas] ?? 10 ** casas; const v = x * f; const r = Math.round(v); return (r - v === 0.5 && r % 2 !== 0 ? r - 1 : r) / f; } const f = 10 ** vba.n(casas); const v = vba.n(x) * f; const r = Math.round(v); const par = Math.abs(v % 1) === 0.5 ? 2 * Math.round(v / 2) : r; return par / f; },"),
    ("cint", &["round"], "  cint(v) { return vba.round(v); },"),
    ("colecao", &["n"], "  /** `New Collection`: itens em ordem, chave opcional (sem diferenciar maiúsculas), índice a partir de 1; `For Each` dá os itens. */\n  colecao() { const e = []; const erro = (n, m) => Object.assign(new Error(m), { number: n }); const pos = (k) => { if (typeof k === 'string') { const c = k.toLowerCase(); const i = e.findIndex((x) => x.k === c); if (i < 0) throw erro(5, `Chamada de procedimento ou argumento inválido: a chave \"${k}\" não está na coleção`); return i; } const i = Math.round(vba.n(k)) - 1; if (i < 0 || i >= e.length) throw erro(9, 'Subscrito fora do intervalo'); return i; }; return { add(v, k, antes, depois) { const c = k === undefined ? undefined : String(k).toLowerCase(); if (c !== undefined && e.some((x) => x.k === c)) throw erro(457, 'Esta chave já está associada a um elemento desta coleção'); const item = { k: c, v }; if (antes !== undefined) e.splice(pos(antes), 0, item); else if (depois !== undefined) e.splice(pos(depois) + 1, 0, item); else e.push(item); }, count() { return e.length; }, item(k) { return e[pos(k)].v; }, remove(k) { e.splice(pos(k), 1); }, set() { throw erro(424, 'O item da coleção não aceita atribuição'); }, [Symbol.iterator]() { return e.map((x) => x.v)[Symbol.iterator](); } }; },"),
    ("dicionario", &[], "  /** `Scripting.Dictionary`: chave → valor; ler uma chave que não existe a cria vazia, como no VBA; `For Each` dá as chaves; `compareMode = 1` (vbTextCompare) ignora maiúsculas, só com o dicionário vazio; `key(k, nova)` troca a chave. */\n  dicionario() { const m = new Map(), orig = new Map(); let texto = false; const erro = (n, t) => Object.assign(new Error(t), { number: n }); const cru = (k) => (typeof k === 'number' || typeof k === 'boolean') ? k : String(k); const ch = (k) => { const x = cru(k); return texto && typeof x === 'string' ? x.toLowerCase() : x; }; const por = (k, v) => { const c = ch(k); if (!m.has(c)) orig.set(c, cru(k)); m.set(c, v); }; return { get compareMode() { return texto ? 1 : 0; }, set compareMode(v) { if (m.size) throw erro(5, 'Argumento ou chamada de procedimento inválida'); texto = Number(v) === 1; }, add(k, v) { if (m.has(ch(k))) throw erro(457, 'Esta chave já está associada a um elemento desta coleção'); por(k, v); }, count() { return m.size; }, exists(k) { return m.has(ch(k)); }, item(k) { const c = ch(k); if (!m.has(c)) por(k, undefined); return m.get(c); }, set(k, v) { por(k, v); }, key(k, nova) { const c = ch(k); if (!m.has(c)) throw erro(32811, 'Elemento não encontrado'); const n = ch(nova); if (n !== c && m.has(n)) throw erro(457, 'Esta chave já está associada a um elemento desta coleção'); const v = m.get(c); m.delete(c); orig.delete(c); por(nova, v); }, keys() { return [...orig.values()]; }, items() { return [...m.values()]; }, remove(k) { const c = ch(k); if (!m.has(c)) throw erro(32811, 'Elemento não encontrado'); m.delete(c); orig.delete(c); }, removeall() { m.clear(); orig.clear(); }, [Symbol.iterator]() { return orig.values(); } }; },"),
    ("faixaInt", &["cint"], "  /** CInt/CLng/CByte: arredonda para o par e dá o erro 6 (Estouro) fora da faixa do tipo. */\n  faixaInt(v, lo, hi) { const x = vba.cint(v); if (x < lo || x > hi) { const e = new RangeError('Estouro'); e.number = 6; throw e; } return x; },"),
    ("strNum", &["str"], "  /** Str: número não negativo ganha o espaço do sinal (Str(5) = \" 5\"). */\n  strNum(v) { return typeof v === 'number' && v >= 0 ? ' ' + vba.str(v) : vba.str(v); },"),
    ("mod", &["cint"], "  /** Mod do VBA: arredonda os dois e fica com o sinal de quem divide. */\n  mod(a, b) { return vba.cint(a) % vba.cint(b); },"),
    ("divInt", &["cint"], "  /** A barra invertida do VBA: divisão inteira. */\n  divInt(a, b) { return Math.trunc(vba.cint(a) / vba.cint(b)); },"),
    ("format", &["str", "numero", "formatoData", "mascara", "formatDataHora", "cdate"], r#"  /** Format(valor, máscara): números (0 # . , % e literais com \x ou "…") e datas (dd/mm/yyyy, hh:nn, mmmm). */
  format(v, m) {
    const especial = vba._formatoNomeado(v, m);
    if (especial !== undefined) return especial;
    // A máscara é analisada uma vez; e, como é função pura e os dados se repetem (as mesmas
    // datas, os mesmos valores), o texto de cada valor também fica guardado.
    const guardadas = vba._mascaras ?? (vba._mascaras = {});
    let g = guardadas[m];
    if (!g) {
      let r = vba.str(m);
      const nomeados = { fixed: '0.00', standard: '#,##0.00', percent: '0.00%', currency: 'R$ #,##0.00', 'medium date': 'dd-mmm-yy' };
      if (nomeados[r.toLowerCase()]) r = nomeados[r.toLowerCase()];
      const f = vba.mascara(r);
      g = guardadas[m] = { m: r, memo: {}, n: 0, fichas: f, data: !f.some((x) => x.d) && /[dmyhns]/i.test(f.map((x) => x.l ?? '').join('')) };
    }
    const k = typeof v === 'number' || typeof v === 'string' ? typeof v + v : null;
    if (k !== null) { const t = g.memo[k]; if (t !== undefined) return t; if (g.n++ > 50000) { g.memo = {}; g.n = 0; } }
    const t = g.data ? vba.formatoData(v, g.m) : vba.formatoNumero(v, g);
    if (k !== null) g.memo[k] = t;
    return t;
  },
  /** Os formatos com nome do Format que não são máscara (General Number, Scientific, Yes/No, True/False, On/Off e as datas e horas com nome), em pt-BR. `undefined` quando `m` é máscara. */
  _formatoNomeado(v, m) {
    if (typeof m !== 'string') return undefined;
    const nome = m.trim().toLowerCase();
    const datas = { 'general date': 0, 'long date': 1, 'short date': 2, 'long time': 3, 'short time': 4 };
    if (nome in datas) return vba.formatDataHora(v, datas[nome]);
    if (nome === 'medium time') { const x = vba.cdate(v); const s = Math.round((x - Math.floor(x)) * 86400); const h = Math.floor(s / 3600) % 24, mi = Math.floor(s / 60) % 60; return `${String(h % 12 || 12).padStart(2, '0')}:${String(mi).padStart(2, '0')} ${h < 12 ? 'AM' : 'PM'}`; }
    const pares = { 'yes/no': ['Sim', 'Não'], 'true/false': ['Verdadeiro', 'Falso'], 'on/off': ['Ativado', 'Desativado'] };
    if (!(nome in pares) && nome !== 'general number' && nome !== 'scientific') return undefined;
    const x = typeof v === 'boolean' ? (v ? -1 : 0) : vba.numero(v);
    if (Number.isNaN(x)) return vba.str(v);
    if (nome in pares) return pares[nome][x !== 0 ? 0 : 1];
    if (nome === 'general number') return vba.str(x);
    const [mant, exp] = x.toExponential(2).split('e');
    return mant.replace('.', ',') + 'E' + (exp[0] === '-' ? '-' : '+') + exp.replace(/^[+-]/, '').padStart(2, '0');
  },
  formatoNumero(v, g) {
    const m = g.m;
    const fichas = g.fichas;
    if (g.data) return vba.formatoData(v, m);
    let x = vba.numero(v);
    if (Number.isNaN(x) || !fichas.some((f) => f.d)) return vba.str(v);
    if (fichas.some((f) => f.l === '%')) x *= 100;
    const ponto = fichas.findIndex((f) => f.p);
    const inteira = ponto < 0 ? fichas : fichas.slice(0, ponto);
    const decimal = ponto < 0 ? [] : fichas.slice(ponto + 1);
    const casas = decimal.filter((f) => f.d).length;
    let [int, frac = ''] = Math.abs(x).toFixed(casas).split('.');
    int = int.padStart(inteira.filter((f) => f.d === '0').length, '0');
    if (int === '0' && !inteira.some((f) => f.d === '0')) int = '';
    let parte;
    if (inteira.some((f) => f.m)) {
      // Com milhar: literais de antes, o número agrupado, literais de depois.
      const lit = (a) => a.map((f) => f.l ?? '').join('');
      const pd = inteira.findIndex((f) => f.d);
      const ud = inteira.length - 1 - [...inteira].reverse().findIndex((f) => f.d);
      parte = lit(inteira.slice(0, pd)) + int.replace(/\B(?=(\d{3})+(?!\d))/g, '.') + lit(inteira.slice(ud + 1));
    } else {
      // Os algarismos entram da direita para a esquerda nos 0/#; o que sobrar vai antes do primeiro.
      const digitos = int.split('');
      const saida = [];
      let antes = 0;
      for (let i = inteira.length - 1; i >= 0; i--) {
        const f = inteira[i];
        if (f.d) {
          saida.unshift(digitos.length ? digitos.pop() : '');
          antes = 0;
        } else if (f.l !== undefined) {
          saida.unshift(f.l);
          antes++;
        }
      }
      if (digitos.length) saida.splice(antes, 0, digitos.join(''));
      parte = saida.join('');
    }
    let dec = '';
    let k = 0;
    for (const f of decimal) dec += f.d ? frac[k++] ?? '' : f.l ?? '';
    return (x < 0 ? '-' : '') + parte + (casas ? ',' : '') + dec;
  },"#),
    ("mascara", &[], r#"  /** As fichas de uma máscara do Format: {d:'0'|'#'}, {p} (ponto), {m} (milhar), {l: literal}. */
  mascara(m) {
    const f = [];
    for (let i = 0; i < m.length; i++) {
      const c = m[i];
      if (c === '\\') f.push({ l: m[++i] ?? '' });
      else if (c === '"') { const j = m.indexOf('"', i + 1) < 0 ? m.length : m.indexOf('"', i + 1); f.push({ l: m.slice(i + 1, j) }); i = j; }
      else if (c === '0' || c === '#') f.push({ d: c });
      else if (c === '.') f.push({ p: true });
      else if (c === ',') f.push({ m: true });
      else f.push({ l: c });
    }
    return f;
  },"#),
    ("serial", &[], "  /** A data do VBA é a da planilha: dias desde 30/12/1899, com a hora na fração. As contas são em UTC para a hora local não deslocar o dia. */\n  serial(a, m = 1, d = 1, h = 0, n = 0, s = 0) { return (Date.UTC(a, m - 1, d, h, n, s) - Date.UTC(1899, 11, 30)) / 86400000; },"),
    ("agora", &["serial"], "  /** Now: a data e a hora do relógio local. */\n  agora() { const d = new Date(); return vba.serial(d.getFullYear(), d.getMonth() + 1, d.getDate(), d.getHours(), d.getMinutes(), d.getSeconds()); },"),
    ("cdate", &["serial", "numero", "str"], r#"  /** CDate: número fica; texto dd/mm/aaaa, aaaa-mm-dd (com hh:mm[:ss]) ou só hh:mm vira data. */
  _inteiro(s) { if (!s || s.length > 4) return NaN; for (let k = 0; k < s.length; k++) { const c = s.charCodeAt(k); if (c < 48 || c > 57) return NaN; } return Number(s); },
  _hms(h) { const p = h.split(':').map(vba._inteiro); return p.length >= 2 && p.length <= 3 && !p.some(Number.isNaN) ? p : null; },
  cdate(v) { if (typeof v === 'number') return v; if (typeof v === 'boolean') return v ? -1 : 0; if (v instanceof Date) return vba.serial(v.getFullYear(), v.getMonth() + 1, v.getDate(), v.getHours(), v.getMinutes(), v.getSeconds()); const t = vba.str(v).trim(); const x = vba.numero(t); if (!Number.isNaN(x)) return x; const inteiro = vba._inteiro, hms = vba._hms; let sp = t.indexOf(' '); if (sp < 0) sp = t.indexOf('T'); const data = sp < 0 ? t : t.slice(0, sp); const hora = sp < 0 ? '' : t.slice(sp + 1).trim(); if (!hora && data.indexOf(':') >= 0) { const h = hms(data); if (h) return (h[0] * 3600 + h[1] * 60 + (h[2] ?? 0)) / 86400; } const sep = data.indexOf('/') >= 0 ? '/' : data.indexOf('-') > 0 ? '-' : '.'; const partes = data.split(sep); const p = partes.map(inteiro); const h = hora ? hms(hora) : [0, 0, 0]; if (p.length === 3 && !p.some(Number.isNaN) && h) { let [y, mes, dia] = partes[0].length === 4 ? p : [p[2], p[1], p[0]]; if (y < 100) y += y < 30 ? 2000 : 1900; return vba.serial(y, mes, dia, h[0], h[1], h[2] ?? 0); } throw new TypeError(`Tipo incompatível: "${t}" não é data`); },"#),
    ("dp", &["cdate"], "  /** As partes da data (ler com getUTC…), arredondada ao segundo. */\n  dp(v) { return new Date(Date.UTC(1899, 11, 30) + Math.round(vba.cdate(v) * 86400) * 1000); },"),
    ("formatoData", &["dp", "str"], r#"  /** Format de data: yyyy yy mmmm mmm mm m dddd ddd dd d hh h nn ss; m/mm logo depois da hora é minuto, como no Excel. A máscara é lida uma vez e guardada. */
  _meses: ['janeiro', 'fevereiro', 'março', 'abril', 'maio', 'junho', 'julho', 'agosto', 'setembro', 'outubro', 'novembro', 'dezembro'],
  _dias: ['domingo', 'segunda-feira', 'terça-feira', 'quarta-feira', 'quinta-feira', 'sexta-feira', 'sábado'],
  _2(x) { return x < 10 ? '0' + x : '' + x; },
  formatoData(v, m) { if (typeof m !== 'string') m = vba.str(m); const guardadas = vba._datas ?? (vba._datas = {}); let fichas = guardadas[m]; if (!fichas) { fichas = []; const re = /yyyy|yy|mmmm|mmm|mm|dddd|ddd|dd|hh|nn|ss|m|d|h/gi; let r, i = 0, hora = false; while ((r = re.exec(m))) { if (r.index > i) fichas.push(m.slice(i, r.index)); let k = r[0].toLowerCase(); const minuto = hora && (k === 'mm' || k === 'm'); hora = k[0] === 'h'; if (minuto) k = k === 'mm' ? 'nn' : 'n'; fichas.push({ k }); i = re.lastIndex; } if (i < m.length) fichas.push(m.slice(i)); guardadas[m] = fichas; } const d = vba.dp(v); const meses = vba._meses, dias = vba._dias, p = vba._2; let s = ''; for (const f of fichas) { if (typeof f === 'string') { s += f; continue; } switch (f.k) { case 'yyyy': s += d.getUTCFullYear(); break; case 'yy': s += p(d.getUTCFullYear() % 100); break; case 'mmmm': s += meses[d.getUTCMonth()]; break; case 'mmm': s += meses[d.getUTCMonth()].slice(0, 3); break; case 'mm': s += p(d.getUTCMonth() + 1); break; case 'm': s += d.getUTCMonth() + 1; break; case 'dddd': s += dias[d.getUTCDay()]; break; case 'ddd': s += dias[d.getUTCDay()].slice(0, 3); break; case 'dd': s += p(d.getUTCDate()); break; case 'd': s += d.getUTCDate(); break; case 'hh': s += p(d.getUTCHours()); break; case 'h': s += d.getUTCHours(); break; case 'nn': s += p(d.getUTCMinutes()); break; case 'n': s += d.getUTCMinutes(); break; case 'ss': s += p(d.getUTCSeconds()); break; } } return s; },"#),
    ("weekday", &["cdate", "n"], r#"  /** Weekday(data, primeiro dia): 1 a 7, domingo é 1 (vbSunday). O serial 1 (31/12/1899) foi domingo. */
  weekday(v, p = 1) { const d = (((Math.floor(vba.cdate(v)) + 6) % 7) + 7) % 7; return ((d + 8 - (vba.n(p) || 1)) % 7) + 1; },"#),
    ("dateadd", &["cdate", "dp", "serial", "n"], "  /** DateAdd: d/y/w (dias), ww, h, n, s; m, q e yyyy param no último dia do mês (31/01 + 1 mês = 28/02). */\n  dateadd(i, n, v) { const x = vba.cdate(v); n = Math.trunc(vba.n(n)); const k = String(i).toLowerCase(); const seg = { d: 86400, y: 86400, w: 86400, ww: 604800, h: 3600, n: 60, s: 1 }[k]; if (seg) return (Math.round(x * 86400) + n * seg) / 86400; const meses = { m: 1, q: 3, yyyy: 12 }[k]; if (!meses) throw new Error(`DateAdd: intervalo \"${i}\" inválido`); const d = vba.dp(x); const total = d.getUTCFullYear() * 12 + d.getUTCMonth() + n * meses; const a = Math.floor(total / 12), m = total - a * 12; const ult = new Date(Date.UTC(a, m + 1, 0)).getUTCDate(); return vba.serial(a, m + 1, Math.min(d.getUTCDate(), ult), d.getUTCHours(), d.getUTCMinutes(), d.getUTCSeconds()); },"),
    ("datediff", &["cdate", "dp", "weekday"], "  /** DateDiff: quantas viradas do intervalo há de `a` até `b` (yyyy, q, m, d/y, w, ww, h, n, s). */\n  datediff(i, a, b, p = 1) { const x = vba.cdate(a), y = vba.cdate(b); const da = vba.dp(x), db = vba.dp(y); const s = (v) => Math.round(v * 86400); const sem = (v) => Math.floor((Math.floor(v) - vba.weekday(v, p) + 1) / 7); switch (String(i).toLowerCase()) { case 'yyyy': return db.getUTCFullYear() - da.getUTCFullYear(); case 'q': return (db.getUTCFullYear() - da.getUTCFullYear()) * 4 + Math.floor(db.getUTCMonth() / 3) - Math.floor(da.getUTCMonth() / 3); case 'm': return (db.getUTCFullYear() - da.getUTCFullYear()) * 12 + db.getUTCMonth() - da.getUTCMonth(); case 'd': case 'y': return Math.floor(y) - Math.floor(x); case 'w': return Math.trunc((Math.floor(y) - Math.floor(x)) / 7); case 'ww': return sem(y) - sem(x); case 'h': return Math.floor(s(y) / 3600) - Math.floor(s(x) / 3600); case 'n': return Math.floor(s(y) / 60) - Math.floor(s(x) / 60); case 's': return s(y) - s(x); } throw new Error(`DateDiff: intervalo \"${i}\" inválido`); },"),
    ("datepart", &["dp", "serial", "weekday", "datediff"], "  /** DatePart(intervalo, data): yyyy, q, m, y (dia do ano), d, w, ww, h, n, s. */\n  datepart(i, v, p = 1) { const d = vba.dp(v); const ano = d.getUTCFullYear(); switch (String(i).toLowerCase()) { case 'yyyy': return ano; case 'q': return Math.floor(d.getUTCMonth() / 3) + 1; case 'm': return d.getUTCMonth() + 1; case 'y': return Math.floor(vba.cdate(v)) - vba.serial(ano) + 1; case 'd': return d.getUTCDate(); case 'w': return vba.weekday(v, p); case 'ww': return vba.datediff('ww', vba.serial(ano), v, p) + 1; case 'h': return d.getUTCHours(); case 'n': return d.getUTCMinutes(); case 's': return d.getUTCSeconds(); } throw new Error(`DatePart: intervalo \"${i}\" inválido`); },"),
    ("dataTexto", &["dp", "formatoData"], "  /** A data como a pessoa digitaria na célula (a planilha a reconhece e põe o formato de data). */\n  dataTexto(v) { const x = vba.cdate(v); return vba.formatoData(x, x % 1 ? (Math.floor(x) ? 'dd/mm/yyyy hh:nn:ss' : 'hh:nn:ss') : 'dd/mm/yyyy'); },"),
    ("repetidas", &["coluna", "letra", "n"], "  /** `RemoveDuplicates Columns:=Array(1, 2), Header:=xlYes`: as colunas contam do começo do intervalo; sem `Header` é xlNo. */\n  repetidas(aba, e, cols, cab = 2) { const c0 = vba.coluna(e); const lista = cols === undefined ? undefined : (Array.isArray(cols) ? cols.slice(cols.lb ?? 0) : [cols]).map((c) => vba.letra(c0 + vba.n(c) - 1)); return aba.removeDuplicates(e, lista, { header: Number(cab) === 1 }); },"),
    ("condNova", &["numero"], "  /** `FormatConditions.Add`: a regra fica guardada até o estilo chegar (`.Interior.Color`, `.Font.Bold`). xlCellValue 1 compara com Formula1 (`=\"texto\"`, `=10`), xlExpression 2 é fórmula. */\n  condNova(aba, e, tipo = 1, op = 1, f1, f2) { const ops = { 1: 'between', 2: 'notBetween', 3: 'equal', 4: 'notEqual', 5: 'greaterThan', 6: 'lessThan', 7: 'greaterThanOrEqual', 8: 'lessThanOrEqual' }; const val = (f) => { if (f === undefined) return undefined; const t = String(f).trim(); if (!t.startsWith('=')) return f; const m = t.match(/^=\"((?:[^\"]|\"\")*)\"$/); if (m) return m[1].replace(/\"\"/g, '\"'); const x = vba.numero(t.slice(1)); return Number.isNaN(x) ? t : x; }; const o = Number(tipo) === 2 ? { type: 'formula', formula: String(f1) } : { operator: ops[Number(op)] ?? 'between', value: val(f1), value2: val(f2) }; const r = { aba, e, o }; (vba._regras ?? (vba._regras = [])).push(r); return r; },"),
    ("condAplicar", &[], "  /** Põe na planilha as regras de `FormatConditions.Add` guardadas. */\n  condAplicar() { for (const r of vba._regras ?? []) r.aba.addConditionalFormat(r.e, r.o); vba._regras = []; },"),
    ("condApagar", &["condAplicar"], "  /** `FormatConditions.Delete`: tira as regras que tocam o intervalo. */\n  condApagar(aba, e) { vba.condAplicar(); aba.clearConditionalFormats(e); },"),
    ("potencias", &[], r#"  _pot: [1, 10, 100, 1000, 10000, 100000, 1000000],"#),
    ("strconv", &["str"], r#"  /** StrConv: vbUpperCase 1, vbLowerCase 2, vbProperCase 3 (primeira letra de cada palavra maiúscula; palavra = letras, algarismos e '), vbWide 4 e vbNarrow 8 (largura cheia/meia), vbKatakana 16 e vbHiragana 32; vbUnicode 64 e vbFromUnicode 128 não mudam o texto (o JavaScript já é Unicode). Somados, como no VBA. */
  strconv(s, modo) { s = vba.str(s); modo = Number(modo); const resto = modo & ~3; if (resto) { s = vba.strconv(s, modo & 3); const troca = (de, ate, d) => s.replace(new RegExp(`[${de}-${ate}]`, 'gu'), (c) => String.fromCharCode(c.charCodeAt(0) + d)); if (resto & 4) s = troca('!', '~', 0xFEE0).replace(/ /g, '\u3000'); if (resto & 8) s = troca('\uFF01', '\uFF5E', -0xFEE0).replace(/\u3000/g, ' '); if (resto & 16) s = troca('\u3041', '\u3096', 0x60); if (resto & 32) s = troca('\u30A1', '\u30F6', -0x60); return s; } switch (modo) { case 1: return s.toUpperCase(); case 2: return s.toLowerCase(); case 3: { const memo = vba._proper ?? (vba._proper = { n: 0 }); const ja = memo['k' + s]; if (ja !== undefined) return ja; if (memo.n++ > 50000) vba._proper = { n: 0 }; const b = s.toLowerCase(); let r = '', ult = 0, ant = false; for (let i = 0; i < b.length; i++) { const c = b.charCodeAt(i); const letra = (c >= 97 && c <= 122) || (c >= 48 && c <= 57) || c === 39 || (c >= 223 && c !== 247); if (letra && !ant && c > 96) { r += b.slice(ult, i) + b[i].toUpperCase(); ult = i + 1; } ant = letra; } return (vba._proper['k' + s] = r + b.slice(ult)); } } return s; },"#),
    ("msgBox", &["str", "n"], "  /** MsgBox: os botões do VBA (vbOKCancel 1, vbAbortRetryIgnore 2, vbYesNoCancel 3, vbYesNo 4, vbRetryCancel 5), o ícone (vbCritical 16, vbQuestion 32, vbExclamation 48, vbInformation 64) e o botão padrão (vbDefaultButton2 256, 3 512). Devolve vbOK 1, vbCancel 2, vbAbort 3, vbRetry 4, vbIgnore 5, vbYes 6, vbNo 7. */\n  msgBox(texto, botoes = 0, titulo = '') { const t = (titulo ? vba.str(titulo) + '\\n\\n' : '') + vba.str(texto); const b = vba.n(botoes); const lista = [[1], [1, 2], [3, 4, 5], [6, 7, 2], [6, 7], [4, 2]][b & 7]; if (!lista) throw new Error('MsgBox: botões ' + (b & 7) + ' não existem'); if (lista.length === 1) { alert(t); return 1; } return lista[app.choose(t, lista, b & 0x70, (b >> 8) & 3)]; },"),
    ("inputBoxApp", &["str", "numero", "bool"], "  /** `Application.InputBox(texto, título, padrão, …, Type)`: Cancelar dá False; Type 1 número (texto que não é número: erro 13), 4 lógico, 8 intervalo (o endereço; Cancelar dá o erro 424, como o `Set` do Excel com False); o resto, texto. */\n  inputBoxApp(texto, titulo, padrao, tipo) { const t = tipo === undefined ? 2 : Number(tipo); const r = prompt((vba.vazio(titulo) ? '' : vba.str(titulo) + '\\n\\n') + vba.str(texto), vba.vazio(padrao) ? '' : vba.str(padrao)); const erro = (n, m) => Object.assign(new Error(m), { number: n }); if (r === null || r === undefined) { if (t === 8) throw erro(424, 'Objeto obrigatório'); return false; } if (t === 8) { const e = String(r).trim().replace(/^=/, '').replace(/\\$/g, ''); if (e.includes('!')) throw erro(1004, 'Referência de outra aba no InputBox ainda não é aceita'); return e; } if ((t & 1) && !(t & 2)) { const x = vba.numero(r); if (Number.isNaN(x)) throw erro(13, `Tipo incompatível: \"${r}\" não é número`); return x; } if (t === 4) return vba.bool(r); return r; },"),
    ("inputBox", &["str"], "  inputBox(texto, titulo = '', padrao = '') { return prompt((titulo ? vba.str(titulo) + '\\n\\n' : '') + vba.str(texto), vba.str(padrao)) ?? ''; },"),
    ("midTroca", &["str", "n"], "  /** A instrução `Mid(s, início, [quantos]) = x`: troca os caracteres a partir de `início` pelos de `x`, sem passar do fim de `s` nem mudar o tamanho dele. */\n  midTroca(s, i, q, x) { const t = vba.str(s); const v = vba.str(x); const a = vba.n(i) - 1; if (a < 0 || a >= t.length) { const e = new Error('Argumento ou chamada de procedimento inválida'); e.number = 5; throw e; } let n = Math.min(v.length, t.length - a); if (q !== undefined) n = Math.min(n, Math.max(0, vba.n(q))); return t.slice(0, a) + v.slice(0, n) + t.slice(a + n); },"),
    ("partes", &[], "  /** \"$B$3:C5\" → [linha, coluna, linha, coluna] (do 1). Linha inteira (\"3:5\") e coluna inteira (\"B:D\") também. Várias áreas: a primeira, como o `.Row` e o `.Rows.Count` do Excel. */\n  partes(e) { const ref = (r) => { const m = r.replace(/\\$/g, '').match(/^([A-Z]*)(\\d*)$/i); if (!m) throw new Error(`Endereço inválido: ${e}`); let c = 0; for (const ch of m[1].toUpperCase()) c = c * 26 + ch.charCodeAt(0) - 64; return [m[2] ? Number(m[2]) : 0, c]; }; const [a, b = a] = String(e).split('!').pop().split(',')[0].split(':'); return [...ref(a), ...ref(b)]; },"),
    ("linha", &["partes"], "  /** A linha (do 1) da primeira célula do endereço. */\n  linha(e) { return vba.partes(e)[0] || 1; },"),
    ("coluna", &["partes"], "  /** A coluna (do 1) da primeira célula do endereço. */\n  coluna(e) { return vba.partes(e)[1] || 1; },"),
    ("letra", &[], "  letra(c) { let s = ''; for (let x = c; x > 0; x = Math.floor((x - 1) / 26)) s = String.fromCharCode(65 + ((x - 1) % 26)) + s; return s; },"),
    ("endAbs", &["partes", "letra"], "  /** O `.Address` do VBA: \"$B$3\". */\n  endAbs(e) { return String(e).split(',').map((p) => { const [l0, c0, l1, c1] = vba.partes(p); const um = (l, c) => `$${vba.letra(c)}$${l}`; return l0 === l1 && c0 === c1 ? um(l0, c0) : `${um(l0, c0)}:${um(l1, c1)}`; }).join(','); },"),
    ("faixa", &["partes", "letra"], "  /** Range(a, b): do canto de cima à esquerda ao de baixo à direita. */\n  faixa(a, b) { const p = vba.partes(a); const q = vba.partes(b); return `${vba.letra(Math.min(p[1], q[1]))}${Math.min(p[0], q[0])}:${vba.letra(Math.max(p[3], q[3]))}${Math.max(p[2], q[2])}`; },"),
    ("celula", &["letra"], "  /** Cells(linha, coluna); a coluna pode ser número ou letra. */\n  celula(l, c) { return `${typeof c === 'string' && /^[A-Z]+$/i.test(c) ? c.toUpperCase() : vba.letra(Number(c))}${l}`; },"),
    ("mesclar", &[], "  /** `.Merge`: uma célula só não muda nada no Excel (o motor recusaria). */\n  mesclar(aba, a) { const e = String(a); const p = e.split(':'); if (p.length < 2 || p[0] === p[1]) return; aba.merge(e); },"),
    ("linhas", &["partes"], "  /** `.EntireRow`: \"A88:J92\" → \"88:92\". */\n  linhas(e) { return String(e).split(',').map((p) => { const [l0, , l1] = vba.partes(p); return `${l0}:${l1}`; }).join(','); },"),
    ("colunas", &["partes", "letra"], "  /** `.EntireColumn`: \"B3:D9\" → \"B:D\". */\n  colunas(e) { return String(e).split(',').map((p) => { const [, c0, , c1] = vba.partes(p); return `${vba.letra(c0)}:${vba.letra(c1)}`; }).join(','); },"),
    ("qtdLinhas", &["partes"], "  qtdLinhas(e) { const [l0, , l1] = vba.partes(e); return l1 - l0 + 1; },"),
    ("qtdColunas", &["partes"], "  qtdColunas(e) { const [, c0, , c1] = vba.partes(e); return c1 - c0 + 1; },"),
    ("like", &[], "  /** `texto Like padrão`: * ? # e [lista]. */\n  like(s, p) { const re = p.replace(/[.+^${}()|\\\\]/g, '\\\\$&').replace(/\\*/g, '.*').replace(/\\?/g, '.').replace(/#/g, '\\\\d').replace(/\\[!/g, '[^'); return new RegExp(`^${re}$`).test(s); },"),
    ("contar", &["partes"], "  /** `.Count` do intervalo: quantas células. */\n  contar(e) { let n = 0; for (const p of String(e).split(',')) { const [l0, c0, l1, c1] = vba.partes(p); n += (l1 - l0 + 1) * (c1 - c0 + 1); } return n; },"),
    ("excluir", &["partes"], "  /** `.EntireRow.Delete`/`.EntireColumn.Delete`, também de várias áreas: de baixo para cima, sem excluir a mesma linha duas vezes. */\n  excluir(aba, e, linhas) { const f = String(e).split(',').map((p) => { const q = vba.partes(p); return linhas ? [q[0] || 1, q[2] || 1] : [q[1] || 1, q[3] || 1]; }).sort((a, b) => a[0] - b[0]); const juntas = []; for (const [a, b] of f) { const u = juntas[juntas.length - 1]; if (u && a <= u[1] + 1) u[1] = Math.max(u[1], b); else juntas.push([a, b]); } for (const [a, b] of juntas.reverse()) linhas ? aba.deleteRow(a, b - a + 1) : aba.deleteColumn(a, b - a + 1); },"),
    ("celulas", &[], "  /** Os endereços das células do intervalo (o `For Each c In Range(…)`). */\n  celulas(aba, e) { const v = []; aba.range(e).forEach((_x, _l, _c, end) => v.push(end)); return v; },"),
    ("fim", &["partes", "letra", "vazio"], "  /** `.End(direção)` do VBA (Ctrl+seta): xlUp -4162, xlDown -4121, xlToLeft -4159, xlToRight -4161. */\n  fim(aba, e, dir) { const [l0, c0] = vba.partes(e); const [dl, dc] = { [-4162]: [-1, 0], [-4121]: [1, 0], [-4159]: [0, -1], [-4161]: [0, 1] }[dir]; const MAXL = 1048576, MAXC = 16384; const tem = (l, c) => !vba.vazio(aba.get(`${vba.letra(c)}${l}`)); const ultL = aba.getLastRow(vba.letra(c0)); const ultC = aba.getLastColumn(l0); let l = l0, c = c0; if (dl > 0 && l >= ultL) return `${vba.letra(c)}${MAXL}`; if (dc > 0 && c >= ultC) return `${vba.letra(MAXC)}${l}`; if (dl < 0 && l > ultL) { l = Math.max(ultL, 1); if (ultL) return `${vba.letra(c)}${l}`; } const dentro = (x, y) => x >= 1 && y >= 1 && x <= MAXL && y <= MAXC; if (tem(l, c) && dentro(l + dl, c + dc) && tem(l + dl, c + dc)) { while (dentro(l + dl, c + dc) && tem(l + dl, c + dc)) { l += dl; c += dc; } } else { do { l += dl; c += dc; } while (dentro(l + dl, c + dc) && !tem(l, c)); if (!dentro(l, c)) { l -= dl; c -= dc; } } return `${vba.letra(c)}${l}`; },"),
    ("corIndice", &[], "  /** A cor da paleta antiga do Excel (`ColorIndex`, 1 a 56). */\n  corIndice(i) { const p = ['#000000', '#ffffff', '#ff0000', '#00ff00', '#0000ff', '#ffff00', '#ff00ff', '#00ffff', '#800000', '#008000', '#000080', '#808000', '#800080', '#008080', '#c0c0c0', '#808080', '#9999ff', '#993366', '#ffffcc', '#ccffff', '#660066', '#ff8080', '#0066cc', '#ccccff', '#000080', '#ff00ff', '#ffff00', '#00ffff', '#800080', '#800000', '#008080', '#0000ff', '#00ccff', '#ccffff', '#ccffcc', '#ffff99', '#99ccff', '#ff99cc', '#cc99ff', '#ffcc99', '#3366ff', '#33cccc', '#99cc00', '#ffcc00', '#ff9900', '#ff6600', '#666699', '#969696', '#003366', '#339966', '#003300', '#333300', '#993300', '#993366', '#333399', '#333333']; return p[Number(i) - 1] ?? '#000000'; },"),
    ("semFundo", &[], "  /** Tira a cor de fundo (`Interior.Color = xlNone`). */\n  semFundo(aba, e) { aba.style(e, { bg: undefined }); },"),
    ("alinH", &[], "  /** xlLeft/xlCenter/xlRight/xlGeneral → o nome da API. */\n  alinH(v) { return { [-4131]: 'left', [-4108]: 'center', [-4152]: 'right', 1: 'general', 7: 'center' }[v] ?? 'general'; },"),
    ("alinV", &[], "  alinV(v) { return { [-4160]: 'top', [-4108]: 'middle', [-4107]: 'bottom' }[v] ?? 'bottom'; },"),
    ("matriz", &[], r#"  /** Dim v(n) / Dim v(1 To n, m): o índice é o do VBA (vai até n); `[1, n]` guarda o limite de baixo em `lb`. A última dimensão é criada num laço (a recursão por linha era lenta no motor das macros). */
  matriz(...lim) { const [d, ...resto] = lim; const [lb, n] = Array.isArray(d) ? d : [0, d]; const tam = Math.max(0, n + 1); const a = new Array(tam); if (resto.length === 1) { const [lb2, m] = Array.isArray(resto[0]) ? resto[0] : [0, resto[0]]; const largura = Math.max(0, m + 1); for (let i = 0; i < tam; i++) { const r = new Array(largura); if (lb2) r.lb = lb2; a[i] = r; } } else if (resto.length) { for (let i = 0; i < tam; i++) a[i] = vba.matriz(...resto); } if (lb) a.lb = lb; return a; },"#),
    ("lbound", &[], "  lbound(m, dim = 1) { let a = m; for (let i = 1; i < dim; i++) a = a[a.lb ?? 0]; return a.lb ?? 0; },"),
    ("ler", &["partes", "letra"], "  /** `Range.Value` de várias células: a matriz do VBA, `v(1 To linhas, 1 To colunas)`, lida em blocos (a API lê até 500 mil células por vez). */\n  ler(aba, e) { let [l0, c0, l1, c1] = vba.partes(e); if (l0 === l1 && c0 === c1 && l0) return aba.value(e); if (!l0 || !c0) { const u = aba.getDataRange(); if (!u) return aba.value('A1'); const p = vba.partes(u.address); if (!l0) { l0 = 1; l1 = p[2]; } if (!c0) { c0 = 1; c1 = p[3]; } } const m = [undefined]; m.lb = 1; const passo = Math.max(1, Math.floor(400000 / (c1 - c0 + 1))); for (let l = l0; l <= l1; l += passo) for (const r of aba.range(`${vba.letra(c0)}${l}:${vba.letra(c1)}${Math.min(l1, l + passo - 1)}`).values()) { r.unshift(undefined); r.lb = 1; m.push(r); } return m; },"),
    ("escrever", &["partes", "letra"], "  /** `Range.Value = matriz`: começa no limite de baixo de cada dimensão, recorta ao tamanho da área e grava em blocos. Matriz de uma dimensão é uma linha (repetida em cada linha da área, como no Excel). */\n  escrever(aba, e, v) { if (!Array.isArray(v)) { aba.set(e, v); return; } const [l0, c0, l1, c1] = vba.partes(e); const nl = l0 ? l1 - l0 + 1 : v.length, nc = c0 ? c1 - c0 + 1 : 16384; const linha = (r) => (Array.isArray(r) ? r.slice(r.lb ?? 0, (r.lb ?? 0) + nc) : [r]); const m = Array.isArray(v[v.lb ?? 0]) ? v.slice(v.lb ?? 0, (v.lb ?? 0) + nl).map(linha) : Array.from({ length: nl }, () => linha(v)); const passo = Math.max(1, Math.floor(400000 / Math.max(1, m[0]?.length ?? 1))); for (let i = 0; i < m.length; i += passo) aba.set(`${vba.letra(c0 || 1)}${(l0 || 1) + i}`, m.slice(i, i + passo)); },"),
    ("redim", &["matriz"], "  /** ReDim [Preserve]: a matriz nova, com os valores antigos se `preservar`. */\n  redim(antiga, preservar, ...lim) { const nova = vba.matriz(...lim); if (preservar && Array.isArray(antiga)) antiga.slice(0, nova.length).forEach((x, i) => { nova[i] = x; }); return nova; },"),
    ("ubound", &[], "  ubound(m, dim = 1) { let a = m; for (let i = 1; i < dim; i++) a = a[a.lb ?? 0]; return a.length - 1; },"),
    ("abas", &[], "  /** As abas da pasta, para o `For Each ws In Worksheets`. */\n  abas() { return sheet.sheets().map((n) => wb.sheet(n)); },"),
    ("hoje", &["agora"], "  hoje() { return Math.floor(vba.agora()); },"),
    ("ano", &["dp"], "  ano(v) { return vba.dp(v).getUTCFullYear(); },"),
    ("mes", &["dp"], "  mes(v) { return vba.dp(v).getUTCMonth() + 1; },"),
    ("dia", &["dp"], "  dia(v) { return vba.dp(v).getUTCDate(); },"),
    ("hora", &["dp"], "  hora(v) { return vba.dp(v).getUTCHours(); },"),
    ("minuto", &["dp"], "  minuto(v) { return vba.dp(v).getUTCMinutes(); },"),
    ("segundo", &["dp"], "  segundo(v) { return vba.dp(v).getUTCSeconds(); },"),
    ("arredBaixo", &["n"], "  /** WorksheetFunction.RoundDown. */\n  arredBaixo(x, casas = 0) { const f = 10 ** vba.n(casas); return Math.trunc(vba.n(x) * f) / f; },"),
    ("arredCima", &["n"], "  arredCima(x, casas = 0) { const f = 10 ** vba.n(casas); const v = vba.n(x) * f; return (v < 0 ? Math.floor(v) : Math.ceil(v)) / f; },"),
    ("erro", &[], "  /** O `Err` do VBA: o `catch` guarda o erro; `Err.Number`/`Err.Description` leem. */\n  erro(e) { vba._erro = e; },\n  erroNumero() { return vba._erro ? (vba._erro.number ?? 1) : 0; },\n  erroTexto() { return vba._erro ? String(vba._erro.message ?? vba._erro) : ''; },"),
    ("especiais", &[], "  /** `SpecialCells`: as áreas das células do tipo (\"A2:E2,A4:E9\"); sem nenhuma, o erro 1004 do Excel. */\n  especiais(aba, e, tipo) { const a = aba.specialAreas(e, tipo); if (!a) { const x = new Error('Nenhuma célula foi encontrada.'); x.number = 1004; throw x; } return a; },"),
    ("copiar", &[], "  /** `Range.Copy`/`.Cut` sem destino: guarda a origem para o `Paste` seguinte. */\n  copiar(aba, e, cortar = false) { vba._copia = { aba, e, cortar }; },"),
    ("colar", &[], "  /** `ActiveSheet.Paste` / `Range.PasteSpecial`: cola o último `Copy` (ou move o último `Cut`) a partir da primeira célula de `alvo`. */\n  colar(aba, alvo, modo = 'all') { const c = vba._copia; if (!c) throw new Error('Nada foi copiado antes do Paste'); const destino = String(alvo).split(':')[0]; const mesma = c.aba.name === aba.name; if (c.cortar && mesma) c.aba.move(c.e, destino); else { c.aba.copy(c.e, destino, mesma ? { paste: modo } : { paste: modo, sheet: aba.name }); if (c.cortar) c.aba.clear(c.e); } if (c.cortar) vba._copia = null; },"),
    ("novaPasta", &[], "  /** `Workbooks.Add`: a pasta nova da macro é a lista das abas copiadas para ela (`null` é a aba em branco que o Excel cria); o `SaveAs` grava só as copiadas. */\n  novaPasta() { return { abas: [null] }; },"),
    ("ordem", &[], "  /** O `Worksheet.Sort` do Excel: chaves, intervalo e cabeçalho guardados por aba até o `.Apply`; `filtro` é o intervalo do último `.AutoFilter`. */\n  ordem(aba) { const m = vba._ordens ?? (vba._ordens = {}); return m[aba.name] ?? (m[aba.name] = { chaves: [], faixa: null, cabecalho: 0, filtro: null }); },"),
    ("ordenar", &["ordem", "ordenarFaixa", "partes", "letra"], "  /** `.Sort.Apply`: ordena o `.SetRange`, senão o intervalo do `.AutoFilter`, senão a região da primeira chave (ordenar só a coluna da chave embaralharia as linhas). */\n  ordenar(aba) { const o = vba.ordem(aba); if (!o.chaves.length) return; let f = o.faixa ?? o.filtro; if (!f) { const [l, c] = vba.partes(o.chaves[0][0]); f = aba.getRegion(`${vba.letra(c)}${l}`).address; } vba.ordenarFaixa(aba, f, o.chaves, o.cabecalho); },"),
    ("ordenarFaixa", &["coluna", "letra", "partes", "numero", "vazio"], "  /** Ordena `faixa` por [[intervalo da chave, xlAscending 1 / xlDescending 2]]; cabeçalho xlYes 1, xlNo 2, xlGuess 0 (adivinha: linha 1 texto e linha 2 número). */\n  ordenarFaixa(aba, faixa, chaves, cab = 2) { const keys = chaves.filter(([k]) => k).map(([k, o]) => ({ col: vba.letra(vba.coluna(k)), order: Number(o) === 2 ? 'desc' : 'asc' })); let header = Number(cab) === 1; if (cab === undefined || Number(cab) === 0) { const l = vba.partes(faixa)[0] || 1; header = keys.some(({ col }) => { const a = aba.get(`${col}${l}`), b = aba.get(`${col}${l + 1}`); return typeof a === 'string' && !vba.vazio(a) && Number.isNaN(vba.numero(a)) && !vba.vazio(b) && !Number.isNaN(vba.numero(b)); }); } aba.sort(faixa, keys, { header }); },"),
    ("choose", &["cint"], "  /** Choose(índice, a, b, …): o item do 1; fora da faixa é Null, como no VBA. */\n  choose(i, ...v) { const k = vba.cint(i); return k >= 1 && k <= v.length ? v[k - 1] : null; },"),
    ("switch", &["bool"], "  /** Switch(cond1, valor1, cond2, valor2, …): o valor da primeira condição verdadeira; nenhuma é Null. */\n  switch(...p) { for (let i = 0; i + 1 < p.length; i += 2) if (vba.bool(p[i])) return p[i + 1]; return null; },"),
    ("strcomp", &["str", "n"], "  /** StrComp(a, b, [modo]): -1, 0 ou 1; vbTextCompare (1) ignora maiúsculas; Null com Null. */\n  strcomp(a, b, modo = 0) { if (a === null || b === null) return null; let x = vba.str(a), y = vba.str(b); if (vba.n(modo) === 1) { x = x.toLowerCase(); y = y.toLowerCase(); } return x < y ? -1 : x > y ? 1 : 0; },"),
    ("isDate", &["cdate", "numero", "str"], "  /** IsDate: texto que o CDate entende (número solto não conta, como no VBA). shortcut: data guardada como serial numa variável dá False; tratar quando o tipo Date existir no conversor. */\n  isDate(v) { if (v instanceof Date) return true; if (typeof v !== 'string' || !Number.isNaN(vba.numero(v))) return false; try { vba.cdate(v); return true; } catch (e) { return false; } },"),
    ("hex", &["round"], "  /** Hex/Oct: número arredondado; negativo em complemento de dois (16 bits se cabe no Integer, senão 32). */\n  hex(v, b) { let n = vba.round(v); if (n < 0) n = n >= -32768 ? n & 0xFFFF : n >>> 0; return n.toString(b).toUpperCase(); },"),
    ("nomeMes", &["formatoData", "n"], "  /** MonthName(mês, [abreviado]). */\n  nomeMes(m, abrev = false) { const t = vba._meses[vba.n(m) - 1]; if (t === undefined) throw new RangeError('MonthName: mês fora de 1 a 12'); return abrev ? t.slice(0, 3) : t; },"),
    ("nomeDia", &["formatoData", "n"], "  /** WeekdayName(dia, [abreviado], [primeiro dia]): o dia conta do primeiro dia da semana (vbSunday 1 é o padrão). */\n  nomeDia(d, abrev = false, primeiro = 1) { const p = vba.n(primeiro) || 1; const t = vba._dias[(vba.n(d) - 1 + p - 1) % 7]; if (t === undefined || vba.n(d) < 1 || vba.n(d) > 7) throw new RangeError('WeekdayName: dia fora de 1 a 7'); return abrev ? t.slice(0, 3) : t; },"),
    ("formatNumero", &["format", "n"], "  /** FormatNumber/FormatCurrency/FormatPercent(valor, [casas], [zero à esquerda], [negativo entre parênteses], [agrupar]): casas -1 (padrão) é 2; vbFalse no 3º tira o 0 de ',5'; vbTrue no 4º escreve '(1,00)'; vbFalse no 5º tira o milhar. */\n  formatNumero(v, casas = -1, agrupar = -2, antes = '', depois = '', zero = -2, parenteses = -2) { let c = casas === undefined || casas === null ? -1 : vba.n(casas); if (c < 0) c = 2; const g = agrupar !== 0 && agrupar !== false; const x = vba.numero(v); const negativo = !Number.isNaN(x) && x < 0 && (parenteses === -1 || parenteses === true); let r = vba.format(negativo ? -x : v, antes + (g ? '#,##0' : '0') + (c > 0 ? '.' + '0'.repeat(c) : '') + depois); if (zero === 0 || zero === false) r = r.replace(/(^|[^0-9])0,/, '$1,'); return negativo ? '(' + r + ')' : r; },"),
    ("formatDataHora", &["formatoData", "dp", "cdate", "n"], "  /** FormatDateTime(data, [formato]): vbGeneralDate 0, vbLongDate 1, vbShortDate 2, vbLongTime 3, vbShortTime 4. */\n  formatDataHora(v, f = 0) { const x = vba.cdate(v); const curta = 'dd/mm/yyyy', hora = 'hh:nn:ss'; switch (vba.n(f)) { case 1: { const d = vba.dp(x); return `${vba._dias[d.getUTCDay()]}, ${d.getUTCDate()} de ${vba._meses[d.getUTCMonth()]} de ${d.getUTCFullYear()}`; } case 2: return vba.formatoData(x, curta); case 3: return vba.formatoData(x, hora); case 4: return vba.formatoData(x, 'hh:nn'); default: { const dia = Math.floor(x), fr = x - dia; if (!dia) return vba.formatoData(x, hora); return vba.formatoData(x, fr ? curta + ' ' + hora : curta); } } },"),
    ("fin", &["n"], r#"  /** As financeiras do VBA (as mesmas contas do Excel). `tipo` 1 = pagamento no começo do período. */
  _f(x, d = 0) { return x === undefined || x === null ? d : vba.n(x); },
  _itens(m) { const v = []; const por = (x) => { if (Array.isArray(x)) { for (let i = x.lb ?? 0; i < x.length; i++) por(x[i]); } else if (x !== null && x !== undefined && x !== '') v.push(vba.n(x)); }; por(m); return v; },
  pmt(r, n, pv, fv, t) { r = vba.n(r); n = vba.n(n); pv = vba.n(pv); fv = vba._f(fv); t = vba._f(t); if (!r) return -(pv + fv) / n; const q = (1 + r) ** n; return -(r * (fv + pv * q)) / ((1 + r * t) * (q - 1)); },
  fv(r, n, pmt, pv, t) { r = vba.n(r); n = vba.n(n); pmt = vba.n(pmt); pv = vba._f(pv); t = vba._f(t); if (!r) return -(pv + pmt * n); const q = (1 + r) ** n; return -(pv * q + pmt * (1 + r * t) * (q - 1) / r); },
  pv(r, n, pmt, fv, t) { r = vba.n(r); n = vba.n(n); pmt = vba.n(pmt); fv = vba._f(fv); t = vba._f(t); if (!r) return -(fv + pmt * n); const q = (1 + r) ** n; return -(fv + pmt * (1 + r * t) * (q - 1) / r) / q; },
  nper(r, pmt, pv, fv, t) { r = vba.n(r); pmt = vba.n(pmt); pv = vba.n(pv); fv = vba._f(fv); t = vba._f(t); if (!r) return -(pv + fv) / pmt; const k = pmt * (1 + r * t) / r; return Math.log((k - fv) / (k + pv)) / Math.log(1 + r); },
  ipmt(r, per, n, pv, fv, t) { r = vba.n(r); per = vba.n(per); t = vba._f(t); const p = vba.pmt(r, n, pv, fv, t); if (t === 1 && per === 1) return 0; let j = vba.fv(r, per - 1, p, pv, t) * r; if (t === 1) j /= 1 + r; return j; },
  ppmt(r, per, n, pv, fv, t) { return vba.pmt(r, n, pv, fv, t) - vba.ipmt(r, per, n, pv, fv, t); },
  npv(r, m) { r = vba.n(r); return vba._itens(m).reduce((s, x, i) => s + x / (1 + r) ** (i + 1), 0); },
  _raiz(f, x) { for (let i = 0; i < 100; i++) { const y = f(x), h = 1e-7 * (Math.abs(x) + 1e-7), d = (f(x + h) - y) / h; if (!d) break; const nx = x - y / d; if (Math.abs(nx - x) < 1e-10) return nx; x = nx; } throw new RangeError('Não convergiu (erro 5 do VBA): tente outro palpite'); },
  irr(m, palpite) { const v = vba._itens(m); return vba._raiz((r) => v.reduce((s, x, i) => s + x / (1 + r) ** i, 0), vba._f(palpite, 0.1)); },
  mirr(m, fin, re) { const v = vba._itens(m), n = v.length; fin = vba.n(fin); re = vba.n(re); let pos = 0, neg = 0; v.forEach((x, i) => { if (x > 0) pos += x * (1 + re) ** (n - 1 - i); else neg += x / (1 + fin) ** i; }); return (-pos / neg) ** (1 / (n - 1)) - 1; },
  rate(n, pmt, pv, fv, t, palpite) { return vba._raiz((r) => (Math.abs(r) < 1e-12 ? vba.n(pv) + vba.n(pmt) * vba.n(n) + vba._f(fv) : vba.fv(r, n, pmt, pv, t)), vba._f(palpite, 0.1)); },
  sln(c, s, v) { return (vba.n(c) - vba.n(s)) / vba.n(v); },
  syd(c, s, v, p) { c = vba.n(c); s = vba.n(s); v = vba.n(v); p = vba.n(p); return (c - s) * (v - p + 1) * 2 / (v * (v + 1)); },
  ddb(c, s, v, p, f) { c = vba.n(c); s = vba.n(s); v = vba.n(v); p = vba.n(p); f = vba._f(f, 2); let livro = c, d = 0; for (let i = 1; i <= p; i++) { d = Math.min(livro * f / v, Math.max(0, livro - s)); livro -= d; } return d; },"#),
    ("split", &["str", "n"], "  /** Split(texto, [separador], [limite], [modo]): limite -1 = todas as partes; com limite, a última leva o resto. */\n  split(s, sep = ' ', lim = -1, modo = 0) { const t = vba.str(s), d = vba.str(sep), l = vba.n(lim); if (t === '') return []; if (d === '') return [t]; const alvo = vba.n(modo) === 1 ? t.toLowerCase() : t, dd = vba.n(modo) === 1 ? d.toLowerCase() : d; const r = []; let i = 0, j; while ((l < 0 || r.length < l - 1) && (j = alvo.indexOf(dd, i)) >= 0) { r.push(t.slice(i, j)); i = j + d.length; } r.push(t.slice(i)); return r; },"),
    ("tipo", &["vazio"], r#"  /** TypeName/VarType de um valor (intervalo e aba o conversor resolve antes). Número é Double: a célula do Excel também dá Double. */
  _tipo(v) { if (v === undefined || v === '') return ['Empty', 0]; if (v === null) return ['Null', 1]; if (typeof v === 'number') return ['Double', 5]; if (typeof v === 'string') return ['String', 8]; if (typeof v === 'boolean') return ['Boolean', 11]; if (v instanceof Date) return ['Date', 7]; if (app.isError(v)) return ['Error', 10]; if (Array.isArray(v)) return ['Variant()', 8204]; if (v.__id !== undefined) return ['Worksheet', 9]; return ['Object', 9]; },
"#),
    ("typeName", &["tipo"], "  typeName(v) { return vba._tipo(v)[0]; },"),
    ("varType", &["tipo"], "  varType(v) { return vba._tipo(v)[1]; },"),
    ("isObject", &["tipo"], "  isObject(v) { return vba._tipo(v)[1] === 9; },"),
    ("cverr", &["n"], "  /** CVErr(xlErrNA…): o erro de célula da API (`app.error`). */\n  cverr(n) { const c = { 2000: '#NULL!', 2007: '#DIV/0!', 2015: '#VALUE!', 2023: '#REF!', 2029: '#NAME?', 2036: '#NUM!', 2042: '#N/A' }[vba.n(n)]; if (!c) throw new RangeError('CVErr: código de erro desconhecido ' + n); return app.error(c); },"),
    ("filtrar", &["str", "n"], "  /** Filter(matriz, texto, [incluir], [modo]): os itens que contêm (ou não) o texto, numa matriz do 0. */\n  filtrar(m, p, incluir = true, modo = 0) { const t = vba.n(modo) === 1, q = t ? vba.str(p).toLowerCase() : vba.str(p); const sim = incluir !== false && incluir !== 0; const r = []; for (let i = m.lb ?? 0; i < m.length; i++) { const x = vba.str(m[i]); if ((t ? x.toLowerCase() : x).includes(q) === sim) r.push(x); } return r; },"),
    ("particao", &["n"], "  /** Partition(número, início, fim, intervalo): a faixa como \" 20: 29\" (largura dos números do VBA). */\n  particao(x, a, b, k) { x = Math.trunc(vba.n(x)); a = Math.trunc(vba.n(a)); b = Math.trunc(vba.n(b)); k = Math.trunc(vba.n(k)); if (a < 0 || b <= a || k < 1) { const e = new RangeError('Chamada de procedimento ou argumento inválido'); e.number = 5; throw e; } const w = Math.max(String(b + 1).length, String(a - 1).length); const f = (y) => String(y).padStart(w, ' '); const vazio = ' '.repeat(w); if (x < a) return vazio + ':' + f(a - 1); if (x > b) return f(b + 1) + ':' + vazio; const lo = a + Math.floor((x - a) / k) * k; return f(lo) + ':' + f(Math.min(lo + k - 1, b)); },"),
    ("rnd", &["n"], r#"  /** Rnd com o gerador do próprio VBA (congruencial de 24 bits, semente inicial 327680): a mesma sequência do Excel. Rnd(0) repete o último; Rnd(negativo) recomeça pela semente desse número (Rnd(-1) é sempre 0,224007). */
  rnd(n) {
    let s = vba._semente ?? 0x50000;
    if (n !== undefined && n !== null) {
      n = vba.n(n);
      if (n === 0) return s / 16777216;
      if (n < 0) { const d = new DataView(new ArrayBuffer(4)); d.setFloat32(0, n, true); const u = d.getUint32(0, true); s = (u + (u >>> 24)) & 0xFFFFFF; }
    }
    s = (Math.imul(s, 0x43FD43FD) + 0xC39EC3) & 0xFFFFFF;
    vba._semente = s;
    return s / 16777216;
  },
  /** Randomize [semente]: mistura a semente (sem ela, o Timer) no gerador do Rnd, como o VBA. */
  randomize(n) {
    const x = n === undefined || n === null ? (Date.now() % 86400000) / 1000 : vba.n(n);
    const d = new DataView(new ArrayBuffer(8)); d.setFloat64(0, x, true);
    let h = d.getUint32(4, true);
    h = ((h & 0xFFFF) ^ (h >>> 16)) << 8;
    vba._semente = ((vba._semente ?? 0x50000) & 0xFF) | (h & 0xFFFF00);
  },"#),
    ("ccur", &["n"], "  /** CCur: 4 casas, com o arredondamento do banqueiro (0,00005 → 0; 0,00015 → 0,0002), como o Currency do VBA. */\n  ccur(v) { const f = vba.n(v) * 10000; const t = Math.trunc(f); const meio = Math.abs(Math.abs(f - t) - 0.5) < 1e-7; const r = meio ? (t % 2 === 0 ? t : t + Math.sign(f)) : Math.round(f); return r / 10000; },"),
    ("qbcolor", &["n"], "  /** QBColor(0–15): as 16 cores do QuickBASIC, como número RGB do VBA. */\n  qbcolor(i) { const c = [0, 8388608, 32768, 8421376, 128, 8388736, 32896, 12632256, 8421504, 16711680, 65280, 16776960, 255, 16711935, 65535, 16777215][vba.n(i)]; if (c === undefined) throw new RangeError('QBColor: cor fora de 0 a 15'); return c; },"),
    ("textoErro", &["erro", "n"], "  /** Error(n): o texto do erro do VBA; sem número, o do último erro (Err.Description). */\n  textoErro(n) { if (n === undefined) return vba.erroTexto(); return { 3: 'Return sem GoSub', 5: 'Chamada de procedimento ou argumento inválido', 6: 'Estouro', 7: 'Memória insuficiente', 9: 'Subscrito fora do intervalo', 10: 'Esta matriz é fixa ou está temporariamente bloqueada', 11: 'Divisão por zero', 13: 'Tipo incompatível', 28: 'Espaço de pilha insuficiente', 53: 'Arquivo não encontrado', 91: 'Variável de objeto ou variável do bloco With não definida', 92: 'Loop For não inicializado', 94: 'Uso inválido de Nulo', 424: 'Objeto necessário', 438: 'O objeto não aceita esta propriedade ou método', 1004: 'Erro definido pelo aplicativo ou pelo objeto' }[vba.n(n)] ?? 'Erro definido pelo aplicativo ou pelo objeto'; },"),
    ("achar", &["partes", "letra"], r#"  /** `Range.Find`: depois de `depois` (sem ele, depois da primeira célula, que fica por último, como no Excel), linha a linha, dando a volta. O endereço ou null. */
  achar(aba, faixa, texto, op, depois) {
    vba._busca = { aba, faixa, texto, op };
    const [r0, c0, r1, c1] = vba.partes(faixa);
    const [lr, lc] = depois ? vba.partes(depois) : [r0, c0];
    const L = vba.letra;
    const zona = (a, b, c, d) => (a > c || b > d) ? null : aba.find(String(texto ?? ''), { range: `${L(b)}${a}:${L(d)}${c}`, wholeCell: op.inteira, matchCase: op.maiusculas, inFormulas: op.formulas });
    return zona(lr, lc + 1, lr, c1) ?? zona(lr + 1, c0, r1, c1) ?? zona(r0, c0, lr - 1, c1) ?? zona(lr, c0, lr, lc);
  },"#),
    ("acharProx", &["achar"], "  /** `Range.FindNext(depois)`: a mesma busca do último `Find`. */\n  acharProx(depois) { const b = vba._busca; if (!b) throw new Error('FindNext sem Find antes'); return vba.achar(b.aba, b.faixa, b.texto, b.op, depois); },"),
    ("corVba", &[], "  /** \"#rrggbb\" → a cor do VBA (r + g·256 + b·65536); sem cor, `padrao`. */\n  corVba(h, padrao) { if (!h) return padrao; const n = parseInt(String(h).slice(1, 7), 16); return ((n >> 16) & 255) + ((n >> 8) & 255) * 256 + (n & 255) * 65536; },"),
    ("filtroCrit", &["n"], r#"  /** O critério do `AutoFilter` (Criteria1, Operator, Criteria2) no formato do filtro da planilha: o texto do Excel (">100", "SP", "<>") vale como no CONT.SE; lista para xlFilterValues/Array; top 10; dois critérios com xlAnd (o padrão) ou xlOr. */
  filtroCrit(c1, op, c2) {
    op = op === undefined || op === null ? 0 : vba.n(op);
    const igual = (x) => typeof x !== 'string' || !/^[<>]|[*?~]/.test(x.replace(/^=/, ''));
    const valor = (x) => (typeof x === 'string' ? x.replace(/^=/, '') : x);
    if (Array.isArray(c1)) { const v = []; for (let i = c1.lb ?? 0; i < c1.length; i++) v.push(valor(c1[i])); return v; }
    if (op === 7) return [valor(c1)];
    if (op >= 3 && op <= 6) return { op: 'top10', topo: op === 3 || op === 5, n: vba.n(c1), percentual: op >= 5 };
    if (op > 2) throw new Error(`AutoFilter com Operator ${op} (cor ou filtro dinâmico) ainda não filtra pela macro`);
    if (c2 !== undefined && c2 !== null) {
      if (op === 2 && igual(c1) && igual(c2)) return [valor(c1), valor(c2)];
      return { op: op === 2 ? 'ou' : 'e', val: [c1, c2] };
    }
    return c1 === undefined ? null : c1;
  },"#),
    ("formaEscolher", &[], "  /** `Shapes(…).Select`: a forma do `Selection.ShapeRange` seguinte. */\n  formaEscolher(f) { vba._forma = f; },"),
    ("formaSel", &["formaEscolher"], "  formaSel() { if (!vba._forma) throw new Error('Selection.ShapeRange sem forma selecionada (falta Shapes(\"…\").Select)'); return vba._forma; },"),
    ("validar", &["n"], "  /** `Validation.Add Type, AlertStyle, Operator, Formula1, Formula2` → a validação da API. */\n  validar(aba, e, tipo, alerta = 1, op = 1, f1, f2) { tipo = Number(tipo); if (!tipo) { aba.clearDataValidation(e); return; } const mode = Number(alerta) === 1 ? 'stop' : 'warn'; const t = f1 === undefined ? '' : String(f1).trim(); let regra; if (tipo === 3) { regra = t.startsWith('=') ? { type: 'list', source: t } : { type: 'list', values: t.split(/[,;]/).map((s) => s.trim()).filter((s) => s) }; } else if ((tipo === 1 || tipo === 2) && !t.startsWith('=')) { const o = { 1: 'between', 2: 'notBetween', 3: 'equal', 4: 'notEqual', 5: 'greaterThan', 6: 'lessThan', 7: 'greaterThanOrEqual', 8: 'lessThanOrEqual' }[Number(op)] ?? 'between'; regra = /etween/.test(o) ? { type: 'number', operator: o, min: vba.n(f1), max: vba.n(f2) } : { type: 'number', operator: o, value: vba.n(f1) }; regra.integer = tipo === 1; } else { log(`Validação do Excel (tipo ${tipo}${t.startsWith('=') ? ', com fórmula' : ''}) ainda sem equivalente: ${e} ficou sem regra.`); aba.clearDataValidation(e); return; } regra.mode = mode; aba.setDataValidation(e, regra); },"),
];

/// O bloco `const vba = { … };` com as funções pedidas (e as dependências delas), em ordem
/// fixa. Vazio se nenhuma foi pedida.
pub fn bloco(pedidas: &BTreeSet<&'static str>) -> String {
    let mut todas: BTreeSet<&str> = BTreeSet::new();
    let mut fila: Vec<&str> = pedidas.iter().copied().collect();
    while let Some(n) = fila.pop() {
        if todas.insert(n) {
            if let Some((_, deps, _)) = AJUDAS.iter().find(|(x, _, _)| *x == n) {
                fila.extend(deps.iter().copied());
            }
        }
    }
    if todas.is_empty() {
        return String::new();
    }
    let corpo: Vec<&str> = AJUDAS.iter().filter(|(n, _, _)| todas.contains(n)).map(|(_, _, c)| *c).collect();
    format!("// O jeito do VBA de fazer as coisas, onde o JavaScript faz diferente.\nconst vba = {{\n{}\n}};\n", corpo.join("\n"))
}

/// Todas as ajudas (para os testes).
pub fn todas() -> String {
    bloco(&AJUDAS.iter().map(|(n, _, _)| *n).collect())
}

/// O nome existe na lista (para o gerador não pedir ajuda que não existe).
pub fn existe(nome: &str) -> bool {
    AJUDAS.iter().any(|(n, _, _)| *n == nome)
}

#[cfg(test)]
mod testes {
    use super::*;

    #[test]
    fn leva_as_dependencias_e_nada_mais() {
        let b = bloco(&BTreeSet::from(["eq"]));
        assert!(b.contains("eq(a, b)") && b.contains("cmp(a, b)") && b.contains("vazio(v)"));
        assert!(!b.contains("mid("));
        assert!(bloco(&BTreeSet::new()).is_empty());
    }

    #[test]
    fn toda_dependencia_existe() {
        for (n, deps, _) in AJUDAS {
            for d in *deps {
                assert!(existe(d), "{n} depende de {d}, que não existe");
            }
        }
    }
}
