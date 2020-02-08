use crate::io::SharedTcpStream;
use message::Message;

pub async fn write_message<M>(a: &SharedTcpStream, message: Message<M>) -> Result<(), std::io::Error>{
	a.write_all(message.as_ref()).await
}
