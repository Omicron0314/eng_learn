import Link from "next/link";
export default function NotFound() {
  return (
    <section className="card completion">
      <h1>没有找到这一页</h1>
      <p>回到今天，继续学习。</p>
      <Link className="button" href="/today">
        回到今日
      </Link>
    </section>
  );
}
