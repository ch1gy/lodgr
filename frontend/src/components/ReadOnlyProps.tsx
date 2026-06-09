import type { TicketResponse } from '../api/types';

export function ReadOnlyProps({ ticket }: { ticket: TicketResponse }) {
  return (
    <>
      <div className="lg-props__sec">
        <div className="lg-props__lbl">Properties</div>
        <div className="lg-props__kv">
          <div className="row"><span className="k">Status</span><span className="v italic">{ticket.status}</span></div>
          <div className="row"><span className="k">Priority</span><span className="v italic">{ticket.priority}</span></div>
          <div className="row"><span className="k">Category</span><span className="v italic">{ticket.category ?? '—'}</span></div>
          <div className="row"><span className="k">Due</span><span className="v">{ticket.due_date ?? '—'}</span></div>
          <div className="row"><span className="k">Created</span><span className="v">{new Date(ticket.created_at).toLocaleDateString('en-GB')}</span></div>
        </div>
      </div>
    </>
  );
}
