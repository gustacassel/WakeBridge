import { invoke } from "@tauri-apps/api/core";

interface Dispositivo {
  id: string;
  nome: string;
  sistema: string;
  ip: string | null;
  online: boolean;
  este: boolean;
}

interface Config {
  ponte: string | null;
  porta_ssh: number;
  macs: Record<string, string>;
}

const ESPERA_ACORDAR_MS = 120_000;
const ATUALIZAR_MS = 15_000;

let dispositivos: Dispositivo[] = [];
let config: Config = { ponte: null, porta_ssh: 8022, macs: {} };
const rdp = new Map<string, boolean>();
const acordando = new Set<string>();
let carregando = false;

const $ = <T extends HTMLElement>(seletor: string) => document.querySelector(seletor) as T;
const lista = () => $<HTMLUListElement>("#lista");
const aviso = () => $<HTMLParagraphElement>("#aviso");
const esperar = (ms: number) => new Promise((r) => setTimeout(r, ms));

function el<K extends keyof HTMLElementTagNameMap>(tag: K, classe = "", texto = "") {
  const e = document.createElement(tag);
  if (classe) e.className = classe;
  if (texto) e.textContent = texto;
  return e;
}

function mostrarAviso(texto: string, tipo: "erro" | "ok" = "erro") {
  const a = aviso();
  a.textContent = texto;
  a.className = `aviso ${tipo}`;
  a.hidden = false;
}

const ehWindows = (d: Dispositivo) => d.sistema.toLowerCase() === "windows";

async function carregar() {
  if (carregando) return;
  carregando = true;
  try {
    [dispositivos, config] = await Promise.all([
      invoke<Dispositivo[]>("listar_dispositivos"),
      invoke<Config>("ler_config"),
    ]);
    if (aviso().classList.contains("erro")) aviso().hidden = true;

    // O "online" do Tailscale demora minutos para cair quando o PC dorme; a porta 3389 é a verdade
    await Promise.all(
      dispositivos
        .filter((d) => ehWindows(d) && !d.este && d.ip)
        .map(async (d) => rdp.set(d.id, await invoke<boolean>("rdp_aberto", { ip: d.ip }))),
    );
  } catch (e) {
    mostrarAviso(String(e));
  } finally {
    carregando = false;
    render();
  }
}

async function salvarConfig() {
  try {
    await invoke("salvar_config", { config });
  } catch (e) {
    mostrarAviso(String(e));
    config = await invoke<Config>("ler_config");
  }
  render();
}

async function conectar(d: Dispositivo) {
  try {
    await invoke("conectar_rdp", { ip: d.ip });
  } catch (e) {
    mostrarAviso(String(e));
  }
}

async function acordar(d: Dispositivo) {
  acordando.add(d.id);
  render();
  try {
    await invoke<string>("acordar", { id: d.id });
    const inicio = Date.now();
    while (Date.now() - inicio < ESPERA_ACORDAR_MS) {
      if (await invoke<boolean>("rdp_aberto", { ip: d.ip })) {
        rdp.set(d.id, true);
        const segundos = Math.round((Date.now() - inicio) / 1000);
        mostrarAviso(`${d.nome} acordou em ${segundos} s. Abrindo o RDP.`, "ok");
        await conectar(d);
        return;
      }
      await esperar(3000);
    }
    mostrarAviso(`${d.nome} não respondeu. Ele estava suspenso (e não desligado)?`);
  } catch (e) {
    mostrarAviso(String(e));
  } finally {
    acordando.delete(d.id);
    render();
  }
}

function editarMac(d: Dispositivo) {
  const dialogo = $<HTMLDialogElement>("#dialogo-mac");
  const campo = $<HTMLInputElement>("#mac-valor");
  $("#mac-nome").textContent = d.nome;
  campo.value = config.macs[d.id] ?? "";
  dialogo.onclose = () => {
    if (dialogo.returnValue === "salvar" && campo.value.trim()) {
      config.macs[d.id] = campo.value.trim();
    } else if (dialogo.returnValue === "remover") {
      delete config.macs[d.id];
    } else {
      return;
    }
    salvarConfig();
  };
  dialogo.showModal();
}

function situacao(d: Dispositivo): { classe: string; texto: string } {
  if (acordando.has(d.id)) return { classe: "acordando", texto: "Acordando…" };
  if (d.este) return { classe: "online", texto: "Este computador" };
  if (rdp.get(d.id)) return { classe: "pronto", texto: "RDP disponível" };
  if (d.online) return { classe: "online", texto: "Online" };
  return { classe: "offline", texto: "Offline" };
}

function cartao(d: Dispositivo) {
  const li = el("li", "cartao");
  const { classe, texto } = situacao(d);

  li.append(el("span", `bolinha ${classe}`));

  const info = el("div", "info");
  const nome = el("div", "nome", d.nome);
  if (d.id === config.ponte) nome.append(el("span", "selo", "ponte"));
  info.append(nome, el("div", "detalhe", [texto, d.sistema, d.ip].filter(Boolean).join(" · ")));
  li.append(info);

  const acoes = el("div", "acoes");
  if (ehWindows(d) && !d.este) {
    if (acordando.has(d.id)) {
      const b = el("button", "", "Acordando…");
      b.disabled = true;
      acoes.append(b);
    } else if (rdp.get(d.id)) {
      const b = el("button", "", "Conectar");
      b.onclick = () => conectar(d);
      acoes.append(b);
    } else if (config.macs[d.id]) {
      const b = el("button", "", "Acordar");
      b.disabled = !config.ponte;
      b.title = config.ponte ? "Pede para a ponte enviar o pacote mágico" : "Escolha a ponte lá em cima";
      b.onclick = () => acordar(d);
      acoes.append(b);
    }
    const mac = el("button", "icone", "⚙");
    mac.title = config.macs[d.id] ? `MAC: ${config.macs[d.id]}` : "Configurar MAC para poder acordar";
    mac.onclick = () => editarMac(d);
    acoes.append(mac);
  }
  li.append(acoes);
  return li;
}

function render() {
  const ponte = $<HTMLSelectElement>("#ponte");
  ponte.replaceChildren(el("option", "", "— escolha —"));
  ponte.options[0].value = "";
  for (const d of dispositivos.filter((d) => !d.este)) {
    const o = el("option", "", `${d.nome} (${d.sistema})`);
    o.value = d.id;
    ponte.append(o);
  }
  ponte.value = config.ponte ?? "";

  lista().replaceChildren(...dispositivos.map(cartao));
}

window.addEventListener("DOMContentLoaded", () => {
  $("#atualizar").onclick = () => carregar();
  $<HTMLSelectElement>("#ponte").onchange = (e) => {
    config.ponte = (e.target as HTMLSelectElement).value || null;
    salvarConfig();
  };
  carregar();
  setInterval(() => {
    if (document.visibilityState === "visible" && acordando.size === 0) carregar();
  }, ATUALIZAR_MS);
});
