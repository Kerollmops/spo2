use tide::response::{Response, IntoResponse};

pub enum Either<T, U> {
    Left(T),
    Right(U),
}

impl<T: IntoResponse, U: IntoResponse> IntoResponse for Either<T, U> {
    fn into_response(self) -> Response {
        match self {
            Either::Left(left) => left.into_response(),
            Either::Right(right) => right.into_response(),
        }
    }
}
