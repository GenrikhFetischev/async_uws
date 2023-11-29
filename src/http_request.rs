use uwebsockets_rs::http_request::HttpRequest as SyncHttpRequest;

#[derive(Debug)]
pub struct HttpRequest {
    pub headers: Vec<(String, String)>,
    pub full_url: String,
    pub url: String,
    pub method: String,
    pub case_sensitive_method: String,
    pub parameters: Vec<String>,
}

impl HttpRequest {
    pub fn get_header(&self, header_name: &str) -> Option<&str> {
        self.headers
            .iter()
            .find(|(key, _)| key == header_name)
            .map(|(_, value)| value.as_str())
    }
}

impl From<&mut SyncHttpRequest> for HttpRequest {
    fn from(request: &mut SyncHttpRequest) -> Self {
        let headers = request
            .get_headers()
            .iter()
            .map(|(key, value)| (key.to_string(), value.to_string()))
            .collect();
        let mut parameters = Vec::new();
        let mut param_index = 0;
        while let Some(param) = request.get_parameter(param_index) {
            parameters.push(param.to_owned());
            param_index += 1;
        }

        HttpRequest {
            headers,
            full_url: request.get_full_url().into(),
            url: request.get_url().into(),
            method: request.get_method().into(),
            case_sensitive_method: request.get_case_sensitive_method().into(),
            parameters,
        }
    }
}
