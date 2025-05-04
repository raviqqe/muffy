#[derive(Serialize)]
struct SerializedSuccess {
    response: Option<SerializedResponse>,
}

impl From<Success> for SerializedSuccess {
    fn from(success: Success) -> Self {
        Self {
            response: success
                .response
                .map(|response| SerializedResponse::from_response(&response)),
        }
    }
}
