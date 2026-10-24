import axios from 'axios';
import type {
  ImportacaoEcd,
  ConsultaCnpjResponse,
  GrafoRedeResult,
  MenorCaminhoResult,
  IndicadoresResumo,
  AiChatResponse,
} from '@/types';

/**
 * Cliente HTTP configurado com credenciais para cookies HttpOnly (JWT)
 */
export const apiClient = axios.create({
  baseURL: import.meta?.env?.VITE_API_URL || '/api',
  withCredentials: true,
  headers: {
    'Content-Type': 'application/json',
    Accept: 'application/json',
  },
  timeout: 30000,
});

// Interceptor de resposta para tratamento uniforme de erros
apiClient.interceptors.response.use(
  (response) => response,
  (error) => {
    if (error.response?.status === 401) {
      // Sessão expirada ou não autenticado
      console.warn('Sessão não autorizada ou expirada.');
    }
    return Promise.reject(error);
  }
);

// Endpoints da API
export const api = {
  // Autenticação
  auth: {
    me: async () => {
      const res = await apiClient.get('/auth/me');
      return res.data;
    },
    login: async (credenciais: { email: string; senha: string }) => {
      const res = await apiClient.post('/auth/login', credenciais);
      return res.data;
    },
    logout: async () => {
      const res = await apiClient.post('/auth/logout');
      return res.data;
    },
  },

  // Ingestão e Jobs de Processamento
  importacoes: {
    upload: async (
      file: File,
      onProgress?: (percent: number) => void
    ): Promise<{ job_id: string; hash_arquivo: string; status: string; mensagem: string }> => {
      const formData = new FormData();
      formData.append('arquivo', file);

      const res = await apiClient.post('/importacoes/upload', formData, {
        headers: {
          'Content-Type': 'multipart/form-data',
        },
        onUploadProgress: (progressEvent) => {
          if (progressEvent.total && onProgress) {
            const percent = Math.round((progressEvent.loaded * 100) / progressEvent.total);
            onProgress(percent);
          }
        },
      });
      return res.data;
    },
    listar: async (): Promise<ImportacaoEcd[]> => {
      const res = await apiClient.get<ImportacaoEcd[]>('/importacoes');
      return res.data;
    },
    obter: async (id: string): Promise<ImportacaoEcd> => {
      const res = await apiClient.get<ImportacaoEcd>(`/importacoes/${id}`);
      return res.data;
    },
    obterStreamUrl: (jobId: string): string => {
      const base = import.meta?.env?.VITE_API_URL || '/api';
      return `${base}/jobs/${jobId}/stream`;
    },
  },

  // Auditoria Fiscal e Demonstrativos SPED
  fiscal: {
    consultarCnpj: async (cnpj: string): Promise<ConsultaCnpjResponse> => {
      const clean = cnpj.replace(/\D/g, '');
      const res = await apiClient.get<ConsultaCnpjResponse>(`/fiscal/consulta-cnpj/${clean}`);
      return res.data;
    },
    exportarXlsxUrl: (cnpj: string): string => {
      const clean = cnpj.replace(/\D/g, '');
      const base = import.meta?.env?.VITE_API_URL || '/api';
      return `${base}/fiscal/exportar-xlsx/${clean}`;
    },
    baixarXlsx: async (cnpj: string, razaoSocial?: string): Promise<void> => {
      const clean = cnpj.replace(/\D/g, '');
      const res = await apiClient.get(`/fiscal/exportar-xlsx/${clean}`, {
        responseType: 'blob',
      });
      const blob = new Blob([res.data], {
        type: 'application/vnd.openxmlformats-officedocument.spreadsheetml.sheet',
      });
      const url = window.URL.createObjectURL(blob);
      const link = document.createElement('a');
      link.href = url;
      link.setAttribute('download', `SPED_${clean}_${razaoSocial?.slice(0, 20) || 'auditoria'}.xlsx`);
      document.body.appendChild(link);
      link.click();
      link.remove();
      window.URL.revokeObjectURL(url);
    },
  },

  // Rede Societária GRC
  grc: {
    buscarRede: async (
      raiz: string,
      camadas: number = 2,
      limite: number = 1000
    ): Promise<GrafoRedeResult> => {
      const clean = raiz.replace(/\D/g, '');
      const res = await apiClient.get<GrafoRedeResult>('/grc/rede', {
        params: { raiz: clean || raiz, camadas, limite },
      });
      return res.data;
    },
    menorCaminho: async (origem: string, destino: string): Promise<MenorCaminhoResult> => {
      const res = await apiClient.get<MenorCaminhoResult>('/grc/caminho', {
        params: {
          origem: origem.replace(/\D/g, '') || origem,
          destino: destino.replace(/\D/g, '') || destino,
        },
      });
      return res.data;
    },
  },

  // Indicadores Econômicos
  indicadores: {
    obterResumo: async (cnae?: string, uf?: string): Promise<IndicadoresResumo> => {
      const res = await apiClient.get<IndicadoresResumo>('/indicadores/resumo', {
        params: { cnae, uf },
      });
      return res.data;
    },
  },

  // Copiloto de IA
  ai: {
    enviarMensagem: async (mensagem: string, contexto?: Record<string, unknown>): Promise<AiChatResponse> => {
      const res = await apiClient.post<AiChatResponse>('/ai/chat', {
        mensagem,
        contexto,
      });
      return res.data;
    },
  },
};
