export type StoredAuth = { auth_mode: string; OPENAI_API_KEY?: string | null; tokens?: { access_token?: string; refresh_token?: string; id_token?: string; account_id?: string } };
export function claims(token: string): any {
  try { return JSON.parse(Buffer.from(token.split('.')[1], 'base64url').toString('utf8')); } catch { throw new Error('The saved ChatGPT credential is invalid. Sign in again.'); }
}
export function loginParams(auth: StoredAuth): any {
  if (auth.OPENAI_API_KEY && /api.?key/i.test(auth.auth_mode)) return { type: 'apiKey', apiKey: auth.OPENAI_API_KEY };
  const accessToken = auth.tokens?.access_token;
  if (!accessToken) throw new Error('This account has no access token. Sign in again.');
  const jwt = claims(accessToken);
  const chatgptAccountId = auth.tokens?.account_id ?? jwt['https://api.openai.com/auth']?.chatgpt_account_id;
  if (!chatgptAccountId) throw new Error('This account has no ChatGPT account identity. Sign in again.');
  if (jwt.exp && jwt.exp * 1000 <= Date.now()) throw new Error('This account token expired. Refresh the account or sign in again.');
  return { type: 'chatgptAuthTokens', accessToken, chatgptAccountId, chatgptPlanType: jwt['https://api.openai.com/auth']?.chatgpt_plan_type ?? null };
}
export function matchesIdentity(status: any, auth: StoredAuth) {
  const target = loginParams(auth);
  if (target.type === 'apiKey') return status.authMethod === 'apikey' && status.authToken === target.apiKey;
  if (!status.authToken) return false;
  return claims(status.authToken)['https://api.openai.com/auth']?.chatgpt_account_id === target.chatgptAccountId;
}
