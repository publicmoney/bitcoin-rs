use crate::io::SharedTcpStream;
use message::Message;

pub async fn write_message<M>(stream: &SharedTcpStream, message: Message<M>) -> Result<(), std::io::Error> {
	stream.write_all(message.as_ref()).await
}
